# encoding: utf-8

import struct
import argparse
import array
import json
import typing
import fractions
import os
import bisect
import subprocess


# This is an example of a WAV file header (44 bytes). Data is stored in little-endian byte order.
#
# [Master RIFF chunk]
#    FileTypeBlockID  (4 bytes) : Identifier « RIFF »  (0x52, 0x49, 0x46, 0x46)
#    FileSize        (4 bytes) : Overall file size minus 8 bytes
#    FileFormatID    (4 bytes) : Format = « WAVE »  (0x57, 0x41, 0x56, 0x45)
#
# [Chunk describing the data format]
#    FormatBlocID    (4 bytes) : Identifier « fmt␣ »  (0x66, 0x6D, 0x74, 0x20)
#    BlocSize        (4 bytes) : Chunk size minus 8 bytes, which is 16 bytes here  (0x10)
#    AudioFormat     (2 bytes) : Audio format (1: PCM integer, 3: IEEE 754 float)
#    NbrChannels     (2 bytes) : Number of channels
#    Frequency       (4 bytes) : Sample rate (in hertz)
#    BytePerSec      (4 bytes) : Number of bytes to read per second (Frequency * BytePerBloc).
#    BytePerBloc     (2 bytes) : Number of bytes per block (NbrChannels * BitsPerSample / 8).
#    BitsPerSample   (2 bytes) : Number of bits per sample
#
# [Chunk containing the sampled data]
#    DataBlocID      (4 bytes) : Identifier « data »  (0x64, 0x61, 0x74, 0x61)
#    DataSize        (4 bytes) : SampledData size
#    SampledData


class WavReader:
    def __init__(self, filename):
        self.filename = filename
        self.wav_file = open(filename, "rb")
        # Read channel count, sample rate.
        self.wav_file.seek(22)
        self.channels = struct.unpack("<H", self.wav_file.read(2))[0]
        self.sample_rate = struct.unpack("<I", self.wav_file.read(4))[0]
        self.bytes_per_second = struct.unpack("<I", self.wav_file.read(4))[0]
        self.wav_file.read(2)  # Skip block align.
        self.bytes_per_sample = struct.unpack("<H", self.wav_file.read(2))[0] // 8
        # The offset to the wave data skipping all headers. Assuming 1 chunk.
        self.offset_to_data = 44

    @property
    def total_length(self) -> float:
        """Total length of the WAV file in seconds."""
        return (
            os.path.getsize(self.filename) - self.offset_to_data
        ) / self.bytes_per_second

    def read_raw_bytes(self, start_seconds: float, duraion_seconds=0.125) -> bytes:
        """Read samples starting at `start_seconds` for `duraion_seconds`."""
        start_offset = int(fractions.Fraction(start_seconds) * self.sample_rate) * (
            self.bytes_per_sample * self.channels
        )
        self.wav_file.seek(self.offset_to_data + start_offset)
        sample_count = int(fractions.Fraction(duraion_seconds) * self.sample_rate)
        byte_len = sample_count * (self.bytes_per_sample * self.channels)
        return self.wav_file.read(byte_len)

    def read(self, start_seconds: float, duraion_seconds=0.125) -> array.array:
        """Read samples starting at `start_seconds` for `duraion_seconds`."""
        sample_bytes = self.read_raw_bytes(start_seconds, duraion_seconds)
        # Assuming 16-bit signed PCM.
        assert self.bytes_per_sample == 2
        return array.array("h", sample_bytes)

    def volume(self, samples: array.array):
        # Calculate the volume.
        return sum(map(abs, samples)) / len(samples)

    def find_silence_around(self, candidate_second) -> None | float:
        candidate_second = fractions.Fraction(int(candidate_second * 8), 8)
        # best_vol = 1e10
        # best_second = None
        test_duration = 0.125
        test_backward_seconds = 20
        for offset_int in range(
            0, -int(1 + test_backward_seconds // test_duration), -1
        ):
            offset = offset_int * test_duration
            second = candidate_second + offset
            if second < 0:
                continue
            vol = self.volume(self.read(second, test_duration))
            # print(" Volume at %d: %d" % (offset, vol))
            if vol < 5:
                return second
            # elif vol < best_vol:
            #     best_vol = vol
            #     best_second = second
        # if best_vol < 100:
        #     return best_second
        return None

    def flac_encode(self, start, duraion, out_path, info=None):
        args = [
            os.getenv("flac") or "flac",
        ]
        if info is not None:
            for tag in ("artist", "album", "title"):
                value = info.get(tag)
                if value:
                    args.append("--tag=%s=%s" % (tag.upper(), value.strip()))
        args += [
            "--best",
            "--output-name=%s" % out_path,
            "--sign=signed",
            "--channels=%d" % self.channels,
            "--endian=little",
            "--bps=%d" % (self.bytes_per_sample * 8),
            "--sample-rate=%d" % self.sample_rate,
            "--force-raw-format",
            "--silent",
            "-",
        ]
        proc = subprocess.Popen(args, stdin=subprocess.PIPE)
        sample_bytes = self.read_raw_bytes(start, duraion)
        proc.stdin.write(sample_bytes)
        proc.stdin.close()
        proc.wait()
        if proc.returncode != 0:
            raise RuntimeError("FLAC encoding failed.")


def parse_timestamp_info(filename) -> typing.Iterable[tuple[int, str, object, bool]]:
    with open(filename, "r", encoding="utf-8") as f:
        for line in f:
            try:
                obj = json.loads(line)
                time = obj["time"]
                info = obj["info"]
                title = info["title"]
                album = info["album"]
                artist = info["artist"]
                is_playing = info["status"] == "Playing"
                yield time, f"{artist}/{album}/{title}", info, is_playing
            except Exception:
                pass


def get_file_creation_time(filename):
    return os.path.getctime(filename)


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("filename", help="The input WAV file.")
    parser.add_argument(
        "-t",
        "--timestamps",
        type=str,
        help="The track timestamp info.",
        default="track.log",
    )
    parser.add_argument(
        "-c",
        "--ctime-hint",
        type=int,
        help="The hint of the creation time of the WAV file.",
    )
    opts = parser.parse_args()
    timestamps = list(parse_timestamp_info(opts.timestamps))
    wav_ctime = opts.ctime_hint or get_file_creation_time(opts.filename)
    print("WAV file created at:", wav_ctime)
    wav = WavReader(opts.filename)
    wav_length = wav.total_length
    # bisect in timestamps to find matching start track.
    start = bisect.bisect_left(timestamps, (wav_ctime, ""))
    first = True
    tracks = []
    last_start = 0
    last_info = None
    bad = False
    for time, title, info, is_playing in timestamps[start:]:
        if not is_playing:
            continue
        if first:
            print("Adjust start time by %.2f" % (time - wav_ctime))
            wav_ctime = time
        time = time - wav_ctime
        print("%s %s" % (fmt_time(time), title))
        if time > wav_length:
            print("  End of WAV file.")
            tracks.append((last_start, wav_length - last_start, last_info))
            break
        if not first:
            silence_time = wav.find_silence_around(time)
            if silence_time is not None:
                print(
                    "  Silence gap found at %s %.2f"
                    % (fmt_time(silence_time), silence_time - time)
                )
                tracks.append((last_start, silence_time - last_start, last_info))
                last_start = silence_time
            else:
                print("  NO SILENCE GAP FOUND!")
                bad = True
        last_info = info
        first = False
    if bad:
        print("WARNING: Silence gaps incomplete - Skipped encoding.")
        return
    out_dir = os.path.join("out", os.path.splitext(os.path.basename(wav.filename))[0])
    os.makedirs(out_dir, exist_ok=True)
    for i, (start, duration, info) in enumerate(tracks):
        duration = int(duration * 8 + 7) // 8
        print(f"Encoding {i+1} of {len(tracks)}")
        wav.flac_encode(
            start, duration, os.path.join(out_dir, "%04d.flac" % (i + 1)), info
        )


def fmt_time(seconds: int) -> str:
    h = seconds // 3600
    m = (seconds % 3600) // 60
    s = seconds % 60
    segments = []
    if h:
        segments.append("%d" % h)
    if m or h:
        segments.append("%02d" % m)
    segments.append("%02d.%02d" % (int(s), int(s * 100) % 100))
    return ":".join(segments)


if __name__ == "__main__":
    main()
