"""Configure shared owner login behind Skillet's identity-authorized proxy."""

import asyncio
import os
import tempfile
from pathlib import Path

import yaml
from yaml.nodes import MappingNode, ScalarNode

NETWORKS = ["100.64.0.0/10", "fd7a:115c:a1e0::/48"]
INCLUDE = "skillet-auth.yaml"
HEADER = "# Managed by Skillet: shared Home Assistant owner login.\n"


async def owner_id(root: Path) -> str | None:
    # Read through the pinned application's store API; never edit auth storage.
    if (root / ".storage").is_symlink() or (root / ".storage/auth").is_symlink():
        raise RuntimeError("Refusing linked authentication storage")
    from homeassistant.auth.auth_store import STORAGE_KEY, STORAGE_VERSION
    from homeassistant.core import HomeAssistant
    from homeassistant.helpers.storage import Store

    hass = HomeAssistant(str(root))
    store = Store(hass, STORAGE_VERSION, STORAGE_KEY, private=True, atomic_writes=True)
    data = await store.async_load()
    owners = [
        user
        for user in (data or {}).get("users", [])
        if user.get("is_owner") and user.get("is_active")
    ]
    if len(owners) > 1:
        raise RuntimeError("Ambiguous Home Assistant owner")
    return owners[0]["id"] if owners else None


def providers(user_id: str | None) -> list[dict]:
    fallback = {"type": "homeassistant"}
    if user_id is None:
        # Onboarding creates the owner. Restart once afterward to select it.
        return [fallback]
    return [
        {
            "type": "trusted_networks",
            "trusted_networks": NETWORKS,
            "trusted_users": {network: [user_id] for network in NETWORKS},
            "allow_bypass_login": True,
        },
        fallback,
    ]


def attached_config(text: str) -> str:
    # Compose rather than load: preserve HA !include tags and operator formatting.
    node = yaml.compose(text)
    if not isinstance(node, MappingNode):
        raise RuntimeError("Expected a mapping in configuration.yaml")
    sections = [value for key, value in node.value if key.value == "homeassistant"]
    if not sections:
        return (
            text.rstrip()
            + f"\n\nhomeassistant:\n  auth_providers: !include {INCLUDE}\n"
        )
    if len(sections) != 1 or not isinstance(sections[0], MappingNode):
        raise RuntimeError("Cannot safely attach shared authentication configuration")
    auth = [value for key, value in sections[0].value if key.value == "auth_providers"]
    if (
        len(auth) == 1
        and isinstance(auth[0], ScalarNode)
        and auth[0].tag == "!include"
        and auth[0].value == INCLUDE
    ):
        return text
    raise RuntimeError(
        "Existing homeassistant section: explicitly add "
        f"auth_providers: !include {INCLUDE} there before enabling shared login"
    )


def replace(path: Path, content: str) -> None:
    if path.is_symlink() or (path.exists() and not path.is_file()):
        raise RuntimeError("Refusing unexpected shared authentication file")
    if path.exists() and path.read_text() == content:
        return
    with tempfile.NamedTemporaryFile(mode="w", dir=path.parent, delete=False) as tmp:
        temporary = Path(tmp.name)
        try:
            if path.exists():
                metadata = path.stat()
                os.chown(temporary, metadata.st_uid, metadata.st_gid)
                os.chmod(temporary, metadata.st_mode & 0o7777)
            else:
                os.chmod(temporary, 0o600)
            tmp.write(content)
            tmp.flush()
            os.fsync(tmp.fileno())
            os.replace(temporary, path)
        finally:
            temporary.unlink(missing_ok=True)


def configure(root: Path) -> None:
    config = root / "configuration.yaml"
    if config.is_symlink():
        raise RuntimeError("Refusing linked configuration")
    original = config.read_text()
    text = attached_config(original)
    selected = asyncio.run(owner_id(root))
    include = root / INCLUDE
    if include.is_symlink() or (
        include.exists() and not include.read_text().startswith(HEADER)
    ):
        raise RuntimeError("Refusing an unmanaged authentication include")
    replace(include, HEADER + yaml.safe_dump(providers(selected), sort_keys=False))
    if text != original:
        backup = root / ".skillet-before-shared-auth.yaml"
        if backup.is_symlink() or (backup.exists() and not backup.is_file()):
            raise RuntimeError("Refusing unexpected authentication migration backup")
        try:
            os.link(config, backup)
        except FileExistsError:
            pass
    replace(config, text)
