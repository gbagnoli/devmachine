"""Generate missing Home Assistant state offline, using the pinned image itself."""

import asyncio
import fcntl
import os
import tempfile
from pathlib import Path

HTTP = {
    "server_host": ["127.0.0.1"],
    "server_port": 8123,
    "use_x_forwarded_for": True,
    "trusted_proxies": ["127.0.0.1"],
}


async def initialize_http(root: Path) -> None:
    # Use the pinned application's own schema/store; never synthesize .storage JSON.
    path = root / ".storage/http"
    if (root / ".storage").is_symlink() or path.is_symlink():
        raise RuntimeError("Refusing linked HTTP state")
    if path.exists():
        if not path.is_file():
            raise RuntimeError("Unexpected HTTP state")
        return
    from homeassistant.components.http.config import HTTPConfigStore
    from homeassistant.core import HomeAssistant

    hass = HomeAssistant(str(root))
    store = HTTPConfigStore(hass)
    await store.async_load()
    await store.async_migrate_yaml(HTTP)
    if store.pending is not None:
        await store.async_promote_pending()
    await hass.async_block_till_done()


def initialize(root: Path) -> None:
    os.umask(0o077)
    root.mkdir(mode=0o700, parents=True, exist_ok=True)
    with (root / ".skillet-bootstrap.lock").open("a") as lock:
        fcntl.flock(lock, fcntl.LOCK_EX)
        config = root / "configuration.yaml"
        if config.is_symlink():
            raise RuntimeError("Refusing a linked initial configuration")
        if config.is_file():
            return
        if config.exists():
            raise RuntimeError("Initial configuration is not a regular file")
        # The private upstream generator is deliberately tied to the pinned image.
        from homeassistant.config import _write_default_config

        with tempfile.TemporaryDirectory(
            prefix=".skillet-initialize-", dir=root
        ) as stage:
            staging = Path(stage)
            if not _write_default_config(stage):
                raise RuntimeError("Upstream configuration generation failed")
            asyncio.run(initialize_http(root))
            # Publish dependencies first; the main config is the completion boundary.
            for source in sorted(staging.iterdir()):
                if source.name != "configuration.yaml":
                    publish_missing(source, root / source.name)
            publish_missing(staging / "configuration.yaml", config)


def publish_missing(source: Path, target: Path) -> None:
    if target.is_symlink() or (target.exists() and not target.is_file()):
        raise RuntimeError("Refusing unexpected bootstrap state")
    try:
        os.link(source, target)
    except FileExistsError:
        pass


if __name__ == "__main__":
    initialize(Path("/config"))
