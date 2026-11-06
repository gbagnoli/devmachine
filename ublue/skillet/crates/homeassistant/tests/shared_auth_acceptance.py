"""Guest-only native login checks; execute inside the pinned HA test container."""

import asyncio
from pathlib import Path

import aiohttp

BASE = "http://127.0.0.1:8123"


async def verify() -> None:
    if not Path("/config/skillet-acceptance.txt").is_file():
        raise RuntimeError("Requires the synthetic Home Assistant fixture")
    async with aiohttp.ClientSession(timeout=aiohttp.ClientTimeout(total=15)) as client:
        async with client.get(BASE + "/auth/providers") as response:
            providers = (await response.json())["providers"]
            assert [p["type"] for p in providers] == ["homeassistant"]
        for address in ["127.0.0.1", "192.168.1.10"]:
            async with client.post(
                BASE + "/auth/login_flow",
                headers={"X-Forwarded-For": address},
                json={
                    "client_id": BASE + "/",
                    "redirect_uri": BASE + "/",
                    "handler": ["trusted_networks", None],
                },
            ) as response:
                assert (await response.json())["type"] == "abort"
        selected = []
        for address in ["100.101.102.103", "fd7a:115c:a1e0::1234"]:
            headers = {"X-Forwarded-For": address}
            async with client.get(
                BASE + "/auth/providers", headers=headers
            ) as response:
                providers = (await response.json())["providers"]
                assert [p["type"] for p in providers] == [
                    "trusted_networks",
                    "homeassistant",
                ]
            async with client.post(
                BASE + "/auth/login_flow",
                headers=headers,
                json={
                    "client_id": BASE + "/",
                    "redirect_uri": BASE + "/",
                    "handler": ["trusted_networks", None],
                },
            ) as response:
                flow = await response.json()
                assert flow["type"] == "create_entry", flow.get("type")
                code = flow["result"]
            async with client.post(
                BASE + "/auth/token",
                headers=headers,
                data={
                    "grant_type": "authorization_code",
                    "code": code,
                    "client_id": BASE + "/",
                },
            ) as response:
                tokens = await response.json()
                assert response.status == 200
            try:
                async with client.get(
                    BASE + "/api/",
                    headers={
                        **headers,
                        "Authorization": "Bearer " + tokens["access_token"],
                    },
                ) as response:
                    assert response.status == 200
                async with client.ws_connect(
                    BASE + "/api/websocket", headers=headers
                ) as ws:
                    assert (await ws.receive_json())["type"] == "auth_required"
                    await ws.send_json(
                        {"type": "auth", "access_token": tokens["access_token"]}
                    )
                    assert (await ws.receive_json())["type"] == "auth_ok"
                    await ws.send_json({"id": 1, "type": "auth/current_user"})
                    current = await ws.receive_json()
                    assert current["success"] and current["result"]["is_owner"]
                    selected.append(current["result"]["id"])
            finally:
                async with client.post(
                    BASE + "/auth/revoke",
                    data={"token": tokens["refresh_token"]},
                ) as response:
                    assert response.status == 200
        assert len(set(selected)) == 1
        print(
            "Accepted: IPv4/IPv6 forwarded trusted login selects the same owner, "
            "native API/WebSocket sessions work, local password fallback remains"
        )


if __name__ == "__main__":
    asyncio.run(verify())
