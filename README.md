# sndcat

A command line utility that works on audio streams.

Like [`socat`](https://linux.die.net/man/1/socat), `sndcat` has a concept of
inputs and outputs (or, "source" and "sink"). `sndcat` reads from input,
does some necessary processing (ex. resampling), then writes to outputs.

## Usage

### Examples

Encode a large Podcast file from MP3 to [Opus](https://opus-codec.org/) to
make it smaller. `-o stats()` shows progress. It is optional:

    sndcat -i mp3(podcast.mp3) -o opus(podcast.opus,16000,1,voip) -o stats()

Record a video conference, including what you said and others said:

    # Suppose 5 and 7 are device indexes frm `sndcat list`. One is the
    # microphone recording what you said, and the other is the loopback
    # device recording what others said.
    sndcat -i dev(5) -i dev(7) -o opus(conf.opus)

Keep an output device busy by writing silent samples. This prevents the
device from entering a "paused" state that will make loopback recording
hanging.

    sndcat -i silence() -o dev(6)

### Input

Use `-i` to specify input streams.

An input stream could be from an input device, a mp3 file, a sine wav, or
multiple streams mixed together. It uses a mini DSL. For example:

Reading from the 5-th device. Use `sndcat list` to list available devices:

    -i dev(5)

Mixing two devices. This can be useful for video conferences, and you want
to record both what you said and what others said:

    -i mix(dev(5),dev(7))

Decoding an MP3 file. This can be used to play it, or re-encode into other
formats:

    -i mp3('foo.mp3')

Generating a sine wave at the given frequency. This can be used for testing:

    -i sin(440)

Specifying `-i` multiple times means reading from multiple streams
simultaneously. It is just a syntax sugar of `mix`. For example, the
following are equivalent:

    -i dev(5) -i dev(6) -i sin(440)
    -i mix(dev(5),dev(6),sin(440))

Input streams can be endless. Press Ctrl+C to force end it.

### Output

Use `-o` to specify outputs.

An output could be a device, an [Opus](https://opus-codec.org/) file, or
showing statistics in the terminal.

Writing to the 6-th device. Use `sndcat list` to list available devices:

    -o dev(6)

Encoding into an Opus file:

    -o opus(a.opus,16000,1,voip)   # for voice
    -o opus(a.opus,24000,2,audio)  # for music

Showing simple statistics in terminal. This can be useful to see the
progress of file encoding:

    -o stats()

Specifying `-o` multiple times means writing to them simultaneously.

## Build

### Windows

Install [vcpkg](https://github.com/Microsoft/vcpkg) first. Then build
portaudio in the local `vcpkg` directory:

    vcpkg\build-portaudio.bat

This will build portaudio patched by
[Audicaty](https://github.com/audacity/audacity) which enables loopback
recording.

Then build `sndcat` using Rust toolchain `cargo`:

    cargo install --path .

Currently, Windows is the main platform that `sndcat` is tested.

### Linux

Install dependencies first. For example, For example, Debian/Ubuntu might use
the following command:

    sudo apt install build-essential autoconf portaudio19-dev libopus-dev

Then build `sndcat` using Rust toolchain `cargo`:

    cargo install --path .
