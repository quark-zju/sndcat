import time
import json
import asyncio

from winsdk.windows.media.control import (
    GlobalSystemMediaTransportControlsSessionManager,
)


async def get_media_info():
    sessions = await GlobalSystemMediaTransportControlsSessionManager.request_async()
    current_session = sessions.get_current_session()
    if current_session:
        info = await current_session.try_get_media_properties_async()
        return {"title": info.title, "artist": info.artist, "album": info.album_title}


async def main():
    last_info = None
    while True:
        info = await get_media_info()
        if last_info != info:
            s = json.dumps({"time": time.time(), "info": info})
            with open("track.log", "a") as f:
                f.write(s)
                f.write("\n")
            print(s)
            last_info = info
        await asyncio.sleep(1)


if __name__ == "__main__":
    import asyncio

    asyncio.run(main())
