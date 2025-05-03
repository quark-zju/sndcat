# encoding: utf-8

import ast
import struct
import argparse
import array
import json
import typing
import fractions
import os
import subprocess
import re


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

    def max_volume(self, samples: array.array):
        # Calculate the "max" volume.
        return max(map(abs, samples))

    def find_silence_around(
        self,
        candidate_second,
        threshold=15,
        backwards=25,
        out_best=None,
    ) -> None | float:
        candidate_second = fractions.Fraction(int(candidate_second * 8), 8)
        best_vol = 1e10
        best_second = None
        test_duration = 0.125
        test_backward_seconds = backwards
        for offset_int in range(
            0, -int(1 + test_backward_seconds // test_duration), -1
        ):
            offset = offset_int * test_duration
            second = candidate_second + offset
            if second < 0:
                continue
            vol = self.max_volume(self.read(second, test_duration))
            # print(" Volume at %d: %d" % (offset, vol))
            if vol < best_vol:
                best_vol = vol
                best_second = second
                if out_best is not None:
                    out_best[:] = (best_vol, best_second)
            if vol < threshold:
                return second
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
        if os.path.exists(out_path):
            os.unlink(out_path)
        proc = subprocess.Popen(args, stdin=subprocess.PIPE)
        sample_bytes = self.read_raw_bytes(start, duraion)
        proc.stdin.write(sample_bytes)
        proc.stdin.close()
        proc.wait()
        if proc.returncode != 0:
            raise RuntimeError("FLAC encoding failed.")


def parse_hh_mm_ss_to_seconds(s: str) -> float:
    result = 0.0
    if "." in s:
        s, frac = s.rsplit(".", 1)
        result += float("0." + frac)
    parts = s.split(":")
    for i, part in enumerate(reversed(parts)):
        result += int(part) * (60**i)
    return result


def parse_gaps_txt(filename: str) -> typing.Iterable[tuple[float, float, object]]:
    gaps = []
    rest = []
    with open(filename, "r") as f:
        lines = list(filter(None, map(str.strip, f)))
        # Parse the gaps.
        for line in lines:
            if line.startswith("#"):
                if line.startswith("###"):
                    rest.clear()
                continue
            # Match "[xxx.xxx - yyy.yyy]"
            matched = re.search(r"\[(\d+\.\d+)\s*-\s*(\d+\.\d+)\]", line)
            if matched:
                start, end = matched.groups()
                start = float(start)
                end = float(end)
                gaps.append((start, end))
            else:
                rest.append(line)

    if len(rest) != len(gaps):
        raise ValueError("Gaps count does not match lines count.")

    for i, line in enumerate(rest):
        if line.startswith("{"):
            try:
                obj = json.loads(line)
            except (json.JSONDecodeError, UnicodeDecodeError):
                obj = ast.literal_eval(line)
            info = obj.get("info", obj)
            title = info["title"].strip()
            album = info["album"]
            artist = info["artist"]
            if not album and " \u2014 " in artist:
                artist, album = artist.split(" \u2014 ", 1)
                info["album"] = album
                info["artist"] = artist
        else:
            info = {
                "title": line.strip(),
                "artist": "",
                "album": "",
            }
        start, end = gaps[i]
        yield start, end - start, info


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("filename", help="The input WAV file.")
    parser.add_argument(
        "-s", "--threshold", type=int, help="The silence threshold.", default=120
    )
    parser.add_argument("-f", "--force", action="store_true", help="Force re-analyze.")
    opts = parser.parse_args()
    wav = WavReader(opts.filename)
    wav_length = wav.total_length

    print("WAV length: %s" % fmt_time(wav_length))

    gaps_file = os.path.splitext(wav.filename)[0] + ".gaps.txt"

    # Scan WAV to find silence gaps.
    if opts.force or not os.path.exists(gaps_file):
        gaps = []
        start = 10
        while start + 5 < wav_length:
            best = []
            silence_time = wav.find_silence_around(
                start, opts.threshold, 2, out_best=best
            )
            if silence_time is not None:
                print("Silence gap found at %s" % (fmt_time(silence_time),))
                gaps.append(silence_time)
                start += 8
            start += 2
        gaps.append(wav_length)

        # Write gaps into a file.
        with open(gaps_file, "a") as f:
            f.write("# Found tracks. Edit, delete, merge, or use '#' to comment out.\n")
            for i, end in enumerate(gaps):
                start = gaps[i - 1] if i > 0 else 0
                f.write(f"{i+1}. [{start:.3f} - {end:.3f}] ({fmt_time(end-start)}): \n")
            f.write(
                "### Edit track info below. One per line.\n"
                "### Format: JSON object from windows-track-title.py, or just the title\n"
                "### Lines starting with '#' are ignored.\n"
            )

        # Spawn an editor to edit the gaps file.
        subprocess.check_call(["code", "--wait", gaps_file], shell=True)

    # Parse the gaps file.
    out_dir = os.path.join("out", os.path.splitext(os.path.basename(wav.filename))[0])
    os.makedirs(out_dir, exist_ok=True)
    parsed = list(parse_gaps_txt(gaps_file))
    for i, (start, duration, info) in enumerate(parsed):
        # duration = int(duration * 8 + 7) // 8
        print(f"Encoding {i+1} of {len(parsed)}")
        out_path = os.path.join(out_dir, "part%04d.flac" % (i + 1))
        wav.flac_encode(start, duration, out_path, info)


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
