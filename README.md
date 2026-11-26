# Clueless

A translucent Linux window that listens to a Google Meet or Zoom call, transcribes it with
**Deepgram** and lets Claude answer the questions you are asked. Claude is used through the
official `claude` command with your **claude.ai subscription**, not API tokens.

## Install

```sh
claude auth login     # once, choose your Claude subscription
(umask 077; mkdir -p ~/.config/clueless; cat > ~/.config/clueless/deepgram.key)
                      # once: paste your Deepgram API key, press Enter, then Ctrl+D
./install.sh          # builds and installs to ~/.local/bin
clueless
```

Needs GTK 4.12+, PipeWire or PulseAudio (`pactl`, `parec`), poppler, OpenSSL and Rust.

## Use

1. **Add materials.** Press the paperclip (Ctrl+O) or drop files on the window. Type what Claude
   should do with them, for example *"This is my CV and the job description. Answer as me, briefly."*
   Press Enter.
2. **Open the meeting** in your browser.
3. **Choose the source.** *All system audio* is the default and the most robust. To hear only one
   application, press the refresh button while it is playing sound and pick it from the list.
4. Press **● Listen**. The level bar moves when the call is audible.
5. With **Auto-answer** on, a detected question is answered once the other side stops talking.
   **Answer now** (Ctrl+Enter, or Shift) answers on demand. If there is no question yet, it waits for one.
6. **📷 Screen** (Space) takes a screenshot of the primary screen and asks Claude to solve or answer
   what it shows (a coding task, a test question, a shared slide), with the call as context. If Claude
   is still replying, the screenshot is sent with your next message instead.

Space and Shift work only while you are not typing in the message field, so they never get in the way
of your text. Press Escape or click anywhere else in the window to leave the field. Shift has to be pressed and
released on its own; Shift+Enter and other combinations do nothing special.

Screenshots use Spectacle on KDE Plasma, which captures the screen marked primary in the display
settings (`grim`, capturing everything, elsewhere). The Clueless window is in the picture if it covers
that screen.

### Pauses and questions

Silence alone never triggers anything. After a question is heard, Clueless waits for about
2.5 seconds of silence, or 5 if the sentence sounds unfinished. Each request repeats the recent
transcript, so a question split by pauses is seen whole. If Claude finds the question incomplete,
nothing is shown and Clueless keeps listening. If the speaker goes on shortly after an answer was
requested, the whole question is answered again. These timings are constants at the top of
`src/gate.rs`.

| Control | Meaning |
| --- | --- |
| Slider in the title bar | Background opacity |
| On top | Keep above other windows (KDE Plasma; elsewhere use the window menu) |
| Mic | Also transcribe your own voice as `You` (use headphones, or it hears the call twice) |
| Language | Set it explicitly. *Multilingual* does not cover Ukrainian or Polish |
| Model | `sonnet` (default), `haiku` (fastest, lightest on limits), `opus` (strongest) |
| Stop / Escape | Interrupt a reply |
| New chat | Forget the conversation, transcript and sent files |

Files: images (PNG, JPEG, GIF, WebP up to 3.7 MB), PDF, DOCX, PPTX, XLSX, ODT/ODP/ODS, and any
UTF-8 text or source file. PDFs over 100 pages or 20 MB are sent as extracted text.

### Global shortcuts

Wayland does not let apps grab keys. In *System Settings → Keyboard → Shortcuts* add a command
shortcut while Clueless is running:

```sh
gapplication action io.github.clueless answer   # or: screenshot, listen, stop
```

## Privacy and limits

- While listening, call **audio is sent to Deepgram** (and your microphone's, if **Mic** is on).
  Clueless does not write it to disk. Deepgram's data policy and pricing apply.
- The transcript **text**, your messages and your files are sent to Claude when a request is made.
- **📷 Screen** sends an image of your whole primary screen to Claude. Check what is on it.
- Claude Code keeps conversation history, including attached files, under `~/.claude/projects/`.
- Requests count against your subscription's usage limits.
- This is an ordinary window: it **is visible** if you share your whole screen.
- Transcribing other people may require their consent, and interviews or exams may forbid
  assistance. Check the rules that apply to you.
- Words appear within about a second; an answer starts a few seconds after the question ends.
  Speech recognition makes mistakes; read answers critically.

## Settings

| Environment variable | Purpose |
| --- | --- |
| `DEEPGRAM_API_KEY` | Used instead of the key file |
| `CLUELESS_CLAUDE` | Path to the `claude` executable |

## Develop

```sh
cargo test
```
