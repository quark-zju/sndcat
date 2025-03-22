import time
import json
import asyncio

from winsdk.windows.media.control import (
    GlobalSystemMediaTransportControlsSessionManager,
)
from winsdk.windows.media.control import (
    GlobalSystemMediaTransportControlsSessionPlaybackStatus as PlaybackStatus,
)


async def get_media_info():
    sessions = await GlobalSystemMediaTransportControlsSessionManager.request_async()
    current_session = sessions.get_current_session()
    if current_session:
        info = await current_session.try_get_media_properties_async()
        playback_info = current_session.get_playback_info()
        status_dict = {
            PlaybackStatus.PLAYING: "Playing",
            PlaybackStatus.PAUSED: "Paused",
            PlaybackStatus.STOPPED: "Stopped",
            PlaybackStatus.CLOSED: "Closed",
            PlaybackStatus.CHANGING: "Changing",
        }
        status = status_dict.get(playback_info.playback_status, "Unknown")
        return {
            "title": info.title,
            "artist": info.artist,
            "album": info.album_title,
            "status": status,
        }


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
