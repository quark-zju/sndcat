"""ASR (Automatic Speech Recognition) using Azure Speech API """

# https://docs.microsoft.com/en-us/azure/cognitive-services/speech-service/get-started-speech-to-text?tabs=windowsinstall&pivots=programming-language-python

import argparse
import azure.cognitiveservices.speech as speechsdk
import keyring
import keyring.cli
import re
import socket
import subprocess
import sys
import threading
import time
import unicodedata

from functools import partial as bind

done = [False]


def is_stopped():
    return done[0]


def mark_stopped():
    done[0] = True


def get_input_stream(port):
    stream_format = speechsdk.audio.AudioStreamFormat(
        samples_per_second=16000, bits_per_sample=16, channels=1
    )
    input_stream = speechsdk.audio.PushAudioInputStream(
        stream_format=stream_format,
    )
    sock = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
    try:
        sock.connect(("127.0.0.1", port))
    except Exception:
        print(
            'Cannot connect to sndcat via local TCP port %s. Is `sndcat -i ... -o "tcp16le(%s)"` running?'
            % (port, port)
        )
        raise

    print("Connected to sndcat port %s" % port)

    def pipe(stream):
        size = 16000 // 10 * 2
        try:
            while not is_stopped():
                buf = sock.recv(size)
                stream.write(buf)
        except Exception as e:
            print("Cannot read from sndcat: %s" % (e,))
            mark_stopped()

    thread = threading.Thread(target=pipe, args=(input_stream,), daemon=True)
    thread.start()

    return input_stream


def read_key(name, hint=None):
    key = keyring.get_password("system", name)
    if key is None:
        if hint:
            print(hint)
        keyring.cli.main(["set", "system", name])
        return read_key(name)
    return key


class ASRTextOutput:
    def recognizing(self, text):
        pass

    def recognized(self, text):
        raise NotImplementedError()

    def close(self):
        pass


class TerminalOutput(ASRTextOutput):
    def __init__(self):
        self._recognizing = ""

    def recognizing(self, text):
        import os

        self._recognizing = text
        termwidth = os.get_terminal_size().columns or 80
        text = truncate_left(text, termwidth, "...")
        sys.stdout.write(f"\r{text}\r")
        sys.stdout.flush()

    def recognized(self, text):
        self._recognizing = ""
        sys.stdout.write(f"\r\x1b[J{text}\n")
        sys.stdout.flush()

    def close(self):
        if self._recognizing:
            sys.stdout.write("\n")
            sys.stdout.flush()


def truncate_left(text, width_limit, prefix):
    chars = list(text)
    width_current = len(prefix)
    result = []
    for ch in reversed(chars):
        if unicodedata.east_asian_width(ch) == "Na":
            width_current += 1
        else:
            width_current += 2
        if width_current >= width_limit:
            return prefix + "".join(reversed(result))
        result.append(ch)
    return text


class LocalTextOutput(ASRTextOutput):
    def __init__(self, path):
        self._path = path
        self._first = True

    def recognized(self, text):
        with open(self._path, "a") as f:
            if self._first:
                import datetime

                now = datetime.datetime.now()
                f.write("\n\n# %s\n" % (now,))
                self._first = False
            f.write("%s\n" % text)


class QuipDocOutput(ASRTextOutput):
    def __init__(self, url):
        import quipclient as quip

        self._client = quip.QuipClient(
            access_token=read_key(
                "quip-api-token",
                hint="Get Quip Token from https://quip.com/api/personal-token",
            )
        )
        thread = self._client.get_thread(url.split("/")[-1])
        self._thread_id = thread["thread"]["id"]
        self._section_id = None
        self._temporary = False
        self._recognized = ""
        self._recognizing = ""
        self._lock = threading.Lock()
        self._done = False
        threading.Thread(target=self._update_thread, daemon=True).start()

    def _update_thread(self):
        retry = 0
        while self._is_running():
            text = ""
            temporary = True
            with self._lock:
                if self._recognized:
                    text = self._recognized
                    self._recognized = ""
                    self._recognizing = ""
                    temporary = False
                elif self._recognizing:
                    text = f"<i>{self._recognizing}</i>"
                    temporary = True
                    self._recognizing = ""
            if text:
                while self._is_running():
                    try:
                        t1 = time.time()
                        self._write(text, temporary=temporary)
                        t2 = time.time()
                        # Quip has rate limit.
                        delay = 2 - (t2 - t1)
                        if delay > 0:
                            time.sleep(delay)
                        retry = 0
                        break
                    except Exception:
                        retry += 1
                        if retry > 3:
                            # Exceed Rate Limit? Wait 1 minute.
                            for _i in range(61):
                                if not self._is_running():
                                    break
                                time.sleep(1)
                            retry = 0

    def _is_running(self):
        return not self._done and not is_stopped()

    def close(self):
        self._done = True
        text = ""
        if self._temporary:
            text = self._recognized
            self._write(text, temporary=False)

    def recognizing(self, text):
        with self._lock:
            self._recognizing = text

    def recognized(self, text):
        with self._lock:
            self._recognized += text + "<br>"

    @property
    def section_id(self):
        if self._section_id is None:
            self._section_id = self._create_section("...")
        return self._section_id

    def _write(self, text, temporary=False):
        if self._temporary:
            operation = self._client.REPLACE_SECTION
        elif self._section_id is None:
            operation = self._client.APPEND
        else:
            operation = self._client.AFTER_SECTION
        doc = self._client.edit_document(
            self._thread_id, text, operation=operation, section_id=self._section_id
        )
        html = doc["html"]
        self._section_id = re.findall("<p id='([^']*)'", html)[-1]
        self._temporary = temporary


def parse_args():
    parser = argparse.ArgumentParser(
        description="ASR sndcat TCP output using Azure Speech API."
    )
    parser.add_argument(
        "--port", type=int, default=3000, help="sndcat tcp16le port (default: 3000)"
    )
    parser.add_argument(
        "--lang",
        type=str,
        default="zh-CN",
        help="ASR language (default: zh-CN)",
    )
    parser.add_argument(
        "--region",
        type=str,
        default="westus",
        help="Azure region (default: westus)",
    )
    parser.add_argument(
        "--txt",
        type=str,
        default="asr-output.txt",
        help="append ASR result to a local text document (default: asr-output.txt)",
    )
    parser.add_argument(
        "--quip", type=str, default=None, help="append ASR result to a Quip document"
    )

    args = parser.parse_args()
    return args


def prepare_outputs(args):
    outs = []
    if sys.stdout.isatty():
        outs.append(TerminalOutput())
    if args.txt:
        print("ASR result will be appended to local text file: %s" % (args.txt,))
        outs.append(LocalTextOutput(args.txt))
    if args.quip:
        print("ASR result will be appended to Quip document: %s" % (args.quip,))
        outs.append(QuipDocOutput(args.quip))
    return outs


def prepare_speech_recognizer(args):
    hint = "See https://docs.microsoft.com/en-us/azure/cognitive-services/speech-service/get-started-speech-to-text"
    subscription = read_key("speech-subscription", hint=hint)
    print("ASR Language: %s" % (args.lang,))
    speech_config = speechsdk.SpeechConfig(
        subscription=subscription,
        region=args.region,
        speech_recognition_language=args.lang,
    )
    input_stream = get_input_stream(args.port)
    audio_config = speechsdk.AudioConfig(
        stream=input_stream,
    )
    speech_recognizer = speechsdk.SpeechRecognizer(
        speech_config=speech_config, audio_config=audio_config
    )
    return speech_recognizer


def main(args):
    recognizer = prepare_speech_recognizer(args)
    outs = prepare_outputs(args)

    def stop(outs=outs):
        mark_stopped()
        recognizer.stop_continuous_recognition()
        for out in outs:
            out.close()

    def on_event(e, name="Event"):
        print("Azure Speech %s: %s" % (name, e))

    def on_stopped(e, name="Stopped"):
        on_event(name, e)
        stop()

    def on_recognizing(e):
        # on_event("Recognizing", e)
        text = e.result.text
        for out in outs:
            try:
                out.recognizing(text)
            except Exception as err:
                print("Error in %r.recognizing: %s" % (out, err))

    def on_recognized(e):
        # on_event("Recognized", e)
        text = e.result.text
        for out in outs:
            try:
                out.recognized(text)
            except Exception as err:
                print("Error in %r.recognized: %s" % (out, err))

    recognizer.recognizing.connect(on_recognizing)
    recognizer.recognized.connect(on_recognized)
    recognizer.session_started.connect(bind(on_event, name="Started"))
    recognizer.session_stopped.connect(on_stopped)
    recognizer.canceled.connect(bind(on_stopped, name="Canceled"))

    recognizer.start_continuous_recognition()
    try:
        while not is_stopped():
            time.sleep(0.5)
    except KeyboardInterrupt:
        print("Stopping on Ctrl+C")
        stop()


if __name__ == "__main__":
    args = parse_args()
    main(args)
