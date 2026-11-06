"""Shared-account configuration preserves application state and proxy boundaries."""

import asyncio
import importlib.util
import sys
import tempfile
import types
import unittest
from pathlib import Path
from unittest.mock import AsyncMock, patch

import yaml

SPEC = importlib.util.spec_from_file_location(
    "shared_auth", Path(__file__).parents[1] / "src/shared_auth.py"
)
assert SPEC is not None and SPEC.loader is not None
AUTH = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(AUTH)
USER = "0123456789abcdef0123456789abcdef"


class SharedAuthTests(unittest.TestCase):
    def test_owner_lookup_is_read_only_and_needs_no_registries(self) -> None:
        class Store:
            def __init__(self, *args: object, **kwargs: object) -> None:
                pass

            async def async_load(self) -> dict:
                return {
                    "users": [
                        {"id": "inactive", "is_owner": True, "is_active": False},
                        {"id": USER, "is_owner": True, "is_active": True},
                    ]
                }

        modules = {}
        definitions: dict[str, dict[str, object]] = {
            "homeassistant.auth.auth_store": {
                "STORAGE_KEY": "auth",
                "STORAGE_VERSION": 1,
            },
            "homeassistant.core": {"HomeAssistant": lambda root: object()},
            "homeassistant.helpers.storage": {"Store": Store},
        }
        for name, values in definitions.items():
            module = types.ModuleType(name)
            module.__dict__.update(values)
            modules[name] = module
        with (
            patch.dict(sys.modules, modules),
            tempfile.TemporaryDirectory() as directory,
        ):
            self.assertEqual(asyncio.run(AUTH.owner_id(Path(directory))), USER)

    def test_only_owner_is_selected_in_both_families_and_fallback_remains(self) -> None:
        providers = AUTH.providers(USER)
        self.assertEqual(providers[-1], {"type": "homeassistant"})
        trusted = providers[0]
        self.assertTrue(trusted["allow_bypass_login"])
        self.assertEqual(trusted["trusted_networks"], AUTH.NETWORKS)
        self.assertEqual(
            trusted["trusted_users"], {network: [USER] for network in AUTH.NETWORKS}
        )
        self.assertNotIn("127.0.0.1", trusted["trusted_networks"])
        self.assertEqual(AUTH.providers(None), [{"type": "homeassistant"}])

    def test_existing_formatting_and_includes_are_preserved(self) -> None:
        original = "# operator settings\ndefault_config:\nautomation: !include automations.yaml\n"
        changed = AUTH.attached_config(original)
        self.assertTrue(changed.startswith(original))
        self.assertEqual(AUTH.attached_config(changed), changed)

    def test_existing_authentication_is_never_overwritten(self) -> None:
        for text in [
            "homeassistant:\n  name: House\n",
            "homeassistant: !include homeassistant.yaml\n",
            "homeassistant:\n  auth_providers:\n    - type: homeassistant\n",
            "homeassistant: {}\nhomeassistant: {}\n",
            "[]",
        ]:
            with self.subTest(text=text), self.assertRaises(RuntimeError):
                AUTH.attached_config(text)

    def test_onboarding_retry_updates_only_managed_include(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            config = root / "configuration.yaml"
            config.write_text("default_config:\n")
            config.chmod(0o640)
            with patch.object(AUTH, "owner_id", AsyncMock(return_value=None)):
                AUTH.configure(root)
            self.assertEqual(
                yaml.safe_load((root / AUTH.INCLUDE).read_text()),
                [{"type": "homeassistant"}],
            )
            attached = config.read_text()
            self.assertEqual(
                (root / ".skillet-before-shared-auth.yaml").read_text(),
                "default_config:\n",
            )
            with patch.object(AUTH, "owner_id", AsyncMock(return_value=USER)):
                AUTH.configure(root)
                before = (root / AUTH.INCLUDE).stat().st_mtime_ns
                AUTH.configure(root)
            self.assertEqual(config.read_text(), attached)
            self.assertEqual(config.stat().st_mode & 0o777, 0o640)
            self.assertEqual((root / AUTH.INCLUDE).stat().st_mtime_ns, before)
            self.assertEqual(
                yaml.safe_load((root / AUTH.INCLUDE).read_text()), AUTH.providers(USER)
            )

    def test_unmanaged_auth_include_is_preserved(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / "configuration.yaml").write_text("default_config:\n")
            include = root / AUTH.INCLUDE
            include.write_text("operator settings")
            with (
                patch.object(AUTH, "owner_id", AsyncMock(return_value=USER)),
                self.assertRaises(RuntimeError),
            ):
                AUTH.configure(root)
            self.assertEqual(include.read_text(), "operator settings")

    def test_linked_files_are_rejected_without_changing_configuration(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            config = root / "configuration.yaml"
            config.write_text("default_config:\n")
            (root / AUTH.INCLUDE).symlink_to(root / "elsewhere")
            with (
                patch.object(AUTH, "owner_id", AsyncMock(return_value=USER)),
                self.assertRaises(RuntimeError),
            ):
                AUTH.configure(root)
            self.assertEqual(config.read_text(), "default_config:\n")


if __name__ == "__main__":
    unittest.main()
