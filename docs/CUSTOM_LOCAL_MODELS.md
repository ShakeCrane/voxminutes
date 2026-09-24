# Custom local model servers

VoxMinutes can connect to **user-run, loopback-only, OpenAI-compatible inference services**. This extends the built-in SenseVoice/X-ASR, OPUS-MT/Hy-MT2 and local GGUF summary options without replacing them. It does not load arbitrary model weights itself: preprocessing, inference, hardware (including Intel NPU/OpenVINO), and model compatibility are managed by the chosen server.

## Configure

1. Start your inference service locally. Use an OpenAI-compatible server supporting the task below. Models do not need to come from the built-in download catalog.
2. Settings → Models → **Custom local models**. Enter a display name, task, server model ID, and API root (for example `http://127.0.0.1:8000/v1`).
3. Press **Test connection**; this checks the local server (GET /models, when supported), not model accuracy or hardware device selection. Save.
4. For ASR or translation, press **启用**. The recorder's ASR selection includes configured custom ASR; the history page can re-transcribe with it. Summary profiles appear as `Custom: <name>` in the meeting-summary dialog's local model chooser.
5. **Important**: start the selected server yourself before recording or generating text. If the selected model fails, VoxMinutes reports the request error rather than claiming a successful transcription. The server is not bundled or launched automatically.

| Task | Required endpoint | Request | Response |
| --- | --- | --- | --- |
| ASR | POST `/audio/transcriptions` | multipart `file=audio.wav` (16 kHz mono PCM16), `model`, optional `language` | JSON `{"text":"..."}` |
| Translation | POST `/chat/completions` | OpenAI-compatible messages + `stream:false`, selected `model` | JSON `choices[0].message.content` |
| Summary | POST `/chat/completions` | OpenAI-compatible messages + `stream:true`, selected `model` | OpenAI-compatible SSE deltas / `[DONE]` |

A minimal ASR response example: `{"text":"你好，世界"}`. For llama.cpp/Ollama use the OpenAI-compatible `/v1` endpoint, not their native API routes. For an OpenVINO model use a compatible local server/bridge; installing OpenVINO alone does not expose the HTTP protocol.

## Security and compatibility

- Only `http://localhost`, `http://127.0.0.1`, and `http://[::1]` service roots (optionally ending in `/v1`) are accepted. Redirects are disabled. No audio or text is sent to a LAN/cloud endpoint by these profiles.
- The configuration stores the model name and endpoint, **not** a copy of weights or API secrets. The user controls which model and CPU/GPU/NPU device their server uses; VoxMinutes does not currently detect or verify the device.
- Custom ASR uses chunk-level final results. Partial streaming from a remote inference server is not yet implemented, and short chunks may trade accuracy against latency. A live-voice sample and offline re-transcription should both be tested.
- In the event the server omits GET /models, a 404/405 indicates connectivity only; test an actual inference before relying on a meeting recording.
- Existing recordings remain in their original folders. Settings → Audio → Recording folder changes the destination for **new** sessions, including audio, transcripts and metadata. Export directory is a separate setting (explicit export path takes precedence).

## Verification checklist

- Run `cargo check --workspace`, `cargo test`, then `cd frontend && pnpm install --ignore-workspace && pnpm build` on a supported development machine.
- Add an ASR profile, switch to it, record and re-transcribe a real audio clip. Add translation and summary profiles and exercise each. Restart and confirm selections survive.
- Change recording folder before a default-device and a custom-device session; inspect `audio.mp4`, `transcripts.json`, `metadata.json`, and the persisted recording folder path.
- Verify existing SenseVoice/X-ASR, OPUS-MT/Hy-MT2 and summary GGUF paths still work, and that an invalid/non-loopback endpoint is rejected.

The branch is submitted for review with the limits above; no claim of a tested NPU execution path is made.
