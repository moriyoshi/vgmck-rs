# vgmck-rs

This is a Rust port of [VGMCK](https://vgmrips.net/forum/viewtopic.php?t=835) (Video Game Music Compiler Kit), which is a MML to VGM compiler originally written by **zzo38**.

The original C implementation is licensed under GPL-3.0-or-later, and so is this project.

The original source code of vgmck is recovered from the Internet Archive: https://web.archive.org/web/20170323112324/http://zzo38computer.org/vgm/

## Tools

### vgmck

Compiles Music Macro Language (MML) files to Video Game Music (VGM) format.

```bash
# Compile MML to VGM
vgmck -i input.mml output.vgm

# Read from stdin
cat input.mml | vgmck output.vgm

# List available sound chips
vgmck -L
```

### vgm2json

Converts VGM/VGZ files to human-readable JSON format for inspection and debugging.

```bash
# Pretty-printed JSON to stdout
vgm2json input.vgm

# Compact JSON output
vgm2json --compact input.vgm

# Write to file
vgm2json input.vgm -o output.json

# VGZ files are automatically decompressed
vgm2json input.vgz
```

#### Output Format

```json
{
  "version": "1.61",
  "header": {
    "total_samples": 330750,
    "loop_samples": 330750,
    "chips": {
      "sn76489": {
        "clock": 3579545,
        "feedback": 9,
        "shift_width": 16
      }
    }
  },
  "gd3": {
    "title": "Track Name",
    "game": "Game Name",
    "composer": "Composer Name"
  },
  "commands": [
    { "cmd": "sn76489_write", "data": 135 },
    { "cmd": "wait", "samples": 22050 },
    { "cmd": "ym2612_write", "port": 0, "reg": 40, "data": 240 },
    { "cmd": "end" }
  ]
}
```

#### Supported Commands

| Command | Fields | Description |
|---------|--------|-------------|
| `sn76489_write` | `data` | SN76489 PSG write |
| `ym2612_write` | `port`, `reg`, `data` | YM2612 (Genesis/Mega Drive) |
| `ym2413_write` | `reg`, `data` | YM2413 (OPLL) |
| `ym2151_write` | `reg`, `data` | YM2151 (OPM) |
| `ym3812_write` | `reg`, `data` | YM3812 (OPL2) |
| `ymf262_write` | `port`, `reg`, `data` | YMF262 (OPL3) |
| `ay8910_write` | `reg`, `data` | AY-3-8910 |
| `nes_apu_write` | `reg`, `data` | NES APU (2A03) |
| `gb_dmg_write` | `reg`, `data` | GameBoy DMG |
| `huc6280_write` | `reg`, `data` | PC Engine / TurboGrafx-16 |
| `pokey_write` | `reg`, `data` | Atari POKEY |
| `qsound_write` | `reg`, `data` | Capcom QSound |
| `wait` | `samples` | Wait N samples (44100 Hz) |
| `data_block` | `block_type`, `size` | PCM data block |
| `seek_pcm` | `offset` | Seek in PCM data bank |
| `end` | - | End of sound data |

## Building

```bash
cargo build --release
```

Binaries will be in `target/release/`:
- `vgmck` - MML compiler
- `vgm2json` - VGM to JSON converter

## Supported Sound Chips

- **Sega**: SN76489 (PSG), YM2612 (Genesis)
- **Yamaha FM**: YM2413, YM2151, YM2203, YM2608, YM2610
- **Yamaha OPL**: YM3812, YM3526, YMF262, YMF278B, Y8950
- **AY-series**: AY-3-8910, AY8930
- **Console**: NES APU, GameBoy DMG, HuC6280, POKEY
- **Arcade**: QSound, K051649, K054539, C140
- **Others**: RF5C68, RF5C164, PWM, MultiPCM, and more

## MML Reference Guide

This section provides a comprehensive reference for the Music Macro Language (MML) syntax supported by vgmck.

### Numeric Values

Numbers can be specified as:
- **Decimal**: Optional `-` or `+` sign followed by digits (e.g., `120`, `-5`, `+3`)
- **Hexadecimal**: Prefix with `$` and use uppercase letters (e.g., `$7F`, `$1A`)

### Top-Level Commands

#### GD3 Metadata Tags

VGM files support GD3 tags for embedded metadata (UTF-16 in VGM, but ASCII/UTF-8/CESU-8 accepted in MML).

| Command | Description |
|---------|-------------|
| `#TITLE` | Set track title (lines 0 & 1 of GD3) |
| `#TITLE-E` | English title only |
| `#TITLE-J` | Japanese title only |
| `#GAME` | Set game name (lines 2 & 3) |
| `#GAME-E` | English game name only |
| `#GAME-J` | Japanese game name only |
| `#SYSTEM` | Set system name (lines 4 & 5) |
| `#SYSTEM-E` | English system name only |
| `#SYSTEM-J` | Japanese system name only |
| `#COMPOSER` | Set composer (lines 6 & 7) |
| `#COMPOSER-E` | English composer only |
| `#COMPOSER-J` | Japanese composer only |
| `#DATE` | Set release date (line 8, format: `yyyy/mm/dd`) |
| `#PROGRAMER` | Set VGM programmer (line 9) |
| `#NOTES` | Set notes field (line 10, can use multiple times) |
| `#TEXT???` | Set custom GD3 line by number |
| `"` | Same as `#NOTES` but allows leading spaces |

#### Chip Selection

```mml
#EX-??? channel_groups parameters
```

Select a sound chip. Channel groups are specified with letters identifying each channel, separated by commas. Optional parameters follow with `letter=value` format.

**Example:**
```mml
#EX-PSG ABC,N H=3579545,F=9
```

#### File and Settings Commands

| Command | Description |
|---------|-------------|
| `;` | Comment (ignored by compiler) |
| `#INCLUDE` | Include another MML file |
| `#EOF` | Stop reading from stdin |
| `#RATE` | Set frame rate in Hz (60 for NTSC, 50 for PAL). Positive enables rate scaling, negative disables it |
| `#VOLUME` | Global volume adjustment (-64 to +192, 32 steps = 2x) |
| `#PITCH-CHANGE` | Set base frequency of "C" notes in decihertz |
| `#LOOP-BASE` | Set loop base header (reduces loop count) |
| `#LOOP-MODIFIER` | Set loop modifier (multiply by N/16) |

#### Musical Scale Configuration

| Command | Description |
|---------|-------------|
| `#SCALE` | Define scale letters (a-j, `.` for gaps, max 32 steps). Default: `c.d.ef.g.a.b` |
| `#EQUAL-TEMPERAMENT` | Apply equal temperament after `#SCALE` |
| `#JUST-INTONATION` | Set note pitches by rational numbers (numerator, denominator pairs) |

#### Debug Commands

| Command | Description |
|---------|-------------|
| `#DEBUG-INPUT-LINES` | Display input lines as they are read |
| `#UNOFFICIAL` | Enable unofficial VGM features (currently no-op) |

### Macro Envelope Definitions

Define macro envelopes with `@???` where `???` is a number 0-255:

```mml
@v0 = { 0 3 5 8 10 }      ; Volume envelope
@EN1 = { 0 4 7 | 0 }      ; Arpeggio with loop point
@W0 = { 0 2 4 6 8 10 }    ; Wave table
```

#### Macro Envelope Syntax

| Syntax | Description |
|--------|-------------|
| `{ }` | Envelope block delimiters |
| Numbers | Direct values in the envelope |
| `[ ]N` | Loop block, repeat N times |
| `\|` | Loop restart point (loops back here at end) |
| `A:B` | Gradient from value A to B |
| `'N` | Slow down - repeat each value N times |
| `"name"` | Name (usually filename for samples) |

#### Macro Types

| Macro | Description |
|-------|-------------|
| `@v` | Software volume envelope |
| `@P` | Software panning envelope |
| `@@` | Tone envelope |
| `@x` | Chip-specific option envelope |
| `@EN` | Arpeggio (semitone offsets) |
| `@M` | Multiplication parameter envelope |
| `@W` | Wave table |
| `@S` | Sample data (with filename) |
| `@SL` | Sample list (map notes to samples) |

### Text Macros

Define text macros with `*` followed by a single ASCII character:

```mml
*A o4 l8 v12              ; Define macro A
A cdefgab *A              ; Use macro A
```

### Music Entry

Music is entered by channel letters (uppercase/lowercase) followed by music commands:

```mml
ABC l8 o4 cdefgab         ; Play on channels A, B, C
a l4 o3 cegc              ; Play on channel a
```

Doubling a letter doubles that track's output.

### Music Commands

#### Notes and Rests

| Command | Description |
|---------|-------------|
| `a b c d e f g h i j` | Play note (h-j available with custom `#SCALE`) |
| `+` | Sharp (after note letter) |
| `-` | Flat (after note letter) |
| `'` | High octave (after note letter) |
| `r` | Rest |
| `w` | Wait (like rest but sends no chip command) |
| `@w` | Wait by frames (optionally with comma and shift count) |
| `n` | Direct note by key number (use comma before length) |

**Note length:** Append a number and/or dots after notes/rests (e.g., `c4`, `c4.`, `c2..`)

#### Octave and Pitch

| Command | Description |
|---------|-------------|
| `o` | Set octave (0 is lowest) |
| `>` | Increment octave |
| `<` | Decrement octave |
| `D` | Set detune amount (0 = normal) |
| `K` | Transpose by semitones |

#### Timing and Length

| Command | Description |
|---------|-------------|
| `l` | Set default note length |
| `t` | Set tempo |
| `@q` | Note quantize (frames before note end to stop) |

#### Note Articulation

| Command | Description |
|---------|-------------|
| `^` | Extend/tie note |
| `&` | Join note to next |
| `/` | Portamento to next note |
| `@/` | Portamento settings: `mode,time,step` (mode: 0=Amiga, 1=glissando) |

#### Volume and Panning

| Command | Description |
|---------|-------------|
| `v` | Set volume (0 = quiet, max depends on chip) |
| `P` | Set panning (0 = center, negative = left, positive = right) |
| `ve` | Hardware volume envelope |

#### Tone and Instrument

| Command | Description |
|---------|-------------|
| `@` | Set tone/instrument (chip-dependent) |
| `@G` | Global chip setting |
| `M` | Set multiplier (chip-dependent) |
| `@W` | Select carrier wave table |
| `@WM` | Select modulator wave table |

#### Arpeggio

| Command | Description |
|---------|-------------|
| `EN` | Activate arpeggio from `@EN` macro |
| `ENOF` | Deactivate arpeggio |

#### Note Events

| Command | Description |
|---------|-------------|
| `NOE0` | Normal note-off behavior |
| `NOE1` | Note-off only on new note/rest |
| `NOE2` | Disable all note-off events |

#### Loops and Structure

| Command | Description |
|---------|-------------|
| `L` | Song loop point (for automatic looping) |
| `[ ]N` | Local repeat block (N times) |
| `\` | Play only on first repeat (between `\` and `]`) |
| `{ }` | Triplet block (2/3 normal length) |

#### Direct Hardware Access

| Command | Description |
|---------|-------------|
| `x` | Direct register write: `address,data` |
| `y` | Direct VGM byte output (use with caution) |

#### Track Control

| Command | Description |
|---------|-------------|
| `?X` | Track questioning - continue if matches track X, else suppress until `?.` |
| `?.` | End track suppression |
| `*X` | Call text macro X |
| `@[ ]` | Auto track switch (e.g., `@[AB]` alternates between A and B) |
| `!` | Stop current track |
| `@!` | Fast forward (skip delays until this point) |

### Chip-Specific Reference

#### SN76489 PSG (Sega, BBC Micro, PCjr, Tandy)

```mml
#EX-PSG square,noise
```

**Channel Groups:** `square` (6), `noise` (2)

**Macro Commands:** `v` (0-15), `P` (-1 to +1)

| Parameter | Default | Description |
|-----------|---------|-------------|
| `F` | 9 | Feedback register (9=SMS2/GG/MD, 3=SC-3000/BBC, 6=SN76494) |
| `H` | 3579545 | Clock rate in Hz |
| `S` | 16 | Shift register (16=SMS2/GG/MD, 15=SC-3000/BBC) |
| `d` | on | Enable /8 clock divider |
| `f` | off | Frequency 0 is 0x400 |
| `n` | off | Output negate flag |
| `s` | on | Enable stereo |

**Noise channel:** Use notes `e`, `f`, `f+` for noise types.

#### OPL2 (Yamaha YM3812)

```mml
#EX-OPL2 melody,Hat,Cymbal,Tom,SD,BD
```

**Channel Groups:** `melody` (18), `Hat` (2), `Cymbal` (2), `Tom` (2), `SD` (2), `BD` (2)

**Macro Commands:** `v` (0-63), `@` (macro), `@G` (0-15)

| Parameter | Default | Description |
|-----------|---------|-------------|
| `H` | 3579545 | Clock rate in Hz |

**Instrument Definition (@x macro):**

| Index | Description |
|-------|-------------|
| 0 | Modulator: Tremolo/Vibrato/Sustain/KSR/Freq.Mul |
| 1 | Carrier: Tremolo/Vibrato/Sustain/KSR/Freq.Mul |
| 2 | Modulator: Key Scaling/Output Level |
| 3 | Carrier: Key Scaling/Output Level |
| 4 | Modulator: Attack/Decay |
| 5 | Carrier: Attack/Decay |
| 6 | Modulator: Sustain/Release |
| 7 | Carrier: Sustain/Release |
| 8 | Modulator: Waveform (0=sine, 1=half, 2=abs, 3=pulse) |
| 9 | Carrier: Waveform |
| 10 | Feedback/Algorithm (bits 3-1=feedback, bit 0=algorithm) |

**@G Settings:** bit0=14¢ vibrato, bit1=4.8dB tremolo, bit2=keyboard split

#### OPLL (Yamaha YM2413)

```mml
#EX-OPLL melody,rhythm
```

**Channel Groups:** `melody` (18), `rhythm` (2)

**Macro Commands:** `v` (0-15), `@` (macro)

| Parameter | Default | Description |
|-----------|---------|-------------|
| `H` | 3579545 | Clock rate in Hz |

**Built-in Instruments (@ command):**

| Value | Instrument | Value | Instrument |
|-------|------------|-------|------------|
| 1 | Violin | 9 | Horn |
| 2 | Guitar | 10 | Synthesizer |
| 3 | Piano | 11 | Harpsichord |
| 4 | Flute | 12 | Vibraphone |
| 5 | Clarinet | 13 | Synth Bass |
| 6 | Oboe | 14 | Acoustic Bass |
| 7 | Trumpet | 15 | Electric Guitar |
| 8 | Organ | 17-31 | With sustain |

#### OPN2 (Yamaha YM2612)

```mml
#EX-OPN2 melody,supplementary
```

**Channel Groups:** `melody` (12), `supplementary` (4)

**Macro Commands:** `@G` (0-15), `@` (macro), `P` (-1 to +1), `v` (0-127)

| Parameter | Default | Description |
|-----------|---------|-------------|
| `H` | 7670454 | Clock rate in Hz |

**Operator Definition (@x macro, per operator):**

| Byte | Format | Description |
|------|--------|-------------|
| 1 | `-SDD MMMM` | D=detune, S=direction, M=frequency multiplier |
| 2 | `-LLL LLLL` | Total level (0=loudest) |
| 3 | `RR-A AAAA` | R=key scale, A=attack rate |
| 4 | `T--D DDDD` | T=tremolo, D=first decay rate |
| 5 | `---D DDDD` | Second decay rate |
| 6 | `LLLL RRRR` | L=decay level, R=release rate |
| 7 | `---- EDAH` | SSG-EG settings |

**Feedback/Algorithm:**
- `--FF FAAA` - F=feedback, A=algorithm (0-7)
- `--TT T-VV` - T=tremolo sensitivity, V=vibrato sensitivity

**Algorithms:** 0=a:b:c:d, 1=(a+b):c:d, 2=(a+(b:c)):d, 3=((a:b)+c):d, 4=(a:b)+(c:d), 5=a:(b+c+d), 6=(a:b)+c+d, 7=a+b+c+d

**@G (LFO):** 0=off, 8-15=LFO frequency (4-72 Hz)

#### OPL3 (Yamaha YMF262)

```mml
#EX-OPL3 two-ops,four-ops,rhythm
```

**Channel Groups:** `two-ops` (36), `four-ops` (12), `rhythm` (2)

**Macro Commands:** `v` (0-63), `P` (-1 to +1), `@` (macro), `@G` (0-15), `@S` (0-32767)

| Parameter | Default | Description |
|-----------|---------|-------------|
| `H` | 14318180 | Clock rate in Hz |

**Four-ops Algorithms:** 0=a:b:c:d, 1=a+(b:c:d), 2=(a:b)+(c:d), 3=a+(b:c)+d

#### PC-Engine / HuC6280

```mml
#EX-PCENGINE normal,FM,noise
```

**Channel Groups:** `normal` (12), `FM` (2), `noise` (4)

**Macro Commands:** `v` (0-31), `P` (-15 to +15), `@W` (macro), `@` (0-7), `@G` (0-255), `@WM` (macro), `M` (1-1023)

| Parameter | Default | Description |
|-----------|---------|-------------|
| `H` | 3579545 | Clock rate in Hz |

**Wave table (@W):** 2, 4, 8, 16, or 32 frames, values 0-31 (or -16 to +15 for FM modulation with @WM).

**FM depth (@):** 0=off, 1=1x, 2=16x, 3=256x, 4-7=fixed modulator period

#### Nintendo Famicom (NES APU)

```mml
#EX-FAMICOM square,triangle,noise
```

**Channel Groups:** `square` (4), `triangle` (2), `noise` (2)

**Macro Commands:** `v` (0-15), `@` (0-3)

| Parameter | Default | Description |
|-----------|---------|-------------|
| `H` | 1789772 | Clock rate (1662607=PAL, 1773448=Dendy) |

**Square duty (@):** 0=12.5%, 1=25%, 2=50%, 3=75%

**Triangle:** No volume control.

**Noise:** Octave 0=long noise, octave 1=short noise.

#### Nintendo GameBoy DMG

```mml
#EX-GAMEBOY square,wavetable,noise
```

**Channel Groups:** `square` (4), `wavetable` (2), `noise` (2)

**Macro Commands:** `v` (0-15), `@` (0-3), `P` (-1 to +1), `@W` (macro), `ve` (-15 to +15)

| Parameter | Default | Description |
|-----------|---------|-------------|
| `H` | 4194304 | Clock rate in Hz |

**Square:** Duty 0-3, volume 0-15, use hardware envelopes (ve).

**Wavetable:** 32-frame waveform (0-15), volume 0-3 only, software envelopes.

**Noise:** Volume 0-15, hardware envelopes.

#### AY-3-8910 (General Instruments)

```mml
#EX-GI-AY square,special
```

**Channel Groups:** `square` (6), `special` (2)

**Macro Commands:** `v` (0-15), `@S` (0-31), `@` (0-31), `M` (any), `ve` (any)

| Parameter | Default | Description |
|-----------|---------|-------------|
| `H` | 1789750 | Clock rate in Hz |
| `T` | 0 | Chip type (0=AY8910, 3=AY8930, 16=YM2149, etc.) |
| `S` | 1 | Octave shift between envelope and note |

**@ bits (special channels):** bit0=square off, bit1=noise off, bit2=hold, bit3=alternate, bit4=direction

#### Atari POKEY

```mml
#EX-POKEY normal,hi-res,filtered
```

**Channel Groups:** `normal` (4), `hi-res` (2), `filtered` (2)

**Macro Commands:** `v` (0-15), `@` (0-7), `M` (-16 to +16)

| Parameter | Default | Description |
|-----------|---------|-------------|
| `H` | 1789772 | Clock rate in Hz |
| `p` | off | 9-bit poly-counters (else 17-bit) |
| `c` | off | 15 KHz clock (else 64 KHz) |

**@ command:** Poly-counter selection (7=pure tones)

#### QSound

```mml
#EX-QSOUND normal
```

**Channel Groups:** `normal` (16)

**Macro Commands:** `v` (0-4095), `@S` (macro), `P` (-16 to +16)

**Sample format:** Signed 8-bit mono

| Parameter | Default | Description |
|-----------|---------|-------------|
| `H` | 4000000 | Clock rate in Hz |

**Note:** Panning must be set for output.

#### NeoGeo Pocket

```mml
#EX-NGP square,special
```

**Channel Groups:** `square` (3), `special` (1)

**Macro Commands:** `v` (0-15), `P` (-15 to +15), `@` (0-1)

| Parameter | Default | Description |
|-----------|---------|-------------|
| `H` | 3072000 | Clock rate in Hz |

**@ (special channel):** 0=tones, 1=noise

**Warning:** Cannot be used with `#EX-PSG` in the same file.

### Built-in Sample Synthesizers

For `@S` macros, filenames starting with `#` use built-in synthesizers:

| Prefix | Description |
|--------|-------------|
| `#d` / `#D` | Direct data (8-bit / 16-bit) |
| `#p` / `#P` | Repeated data (count, value pairs) |
| `#s` / `#S` | Sine wave synthesis |

### Example MML

```mml
; Simple example for SN76489 PSG
#TITLE-E Example Song
#GAME-E Example Game
#SYSTEM-E Sega Master System
#COMPOSER-E Composer Name
#DATE 2024/01/01

#EX-PSG ABC,N H=3579545,F=9,S=16

; Volume envelope
@v0 = { 15 14 13 12 11 10 | 10 }

; Channel A - melody
A l8 o4 t120
A @v0 v15 cdef gabc' bagf edcr

; Channel B - harmony
B l8 o3 t120
B @v0 v12 egeg cece dfdf bdbd

; Channel C - bass
C l4 o2 t120
C @v0 v15 c g c g

; Noise channel
N l4 o0 t120
N v8 f f f f
```

## VGM Specification

This implementation targets VGM version 1.61. For the full specification, see:
https://vgmrips.net/wiki/VGM_Specification

## Deviations from Original vgmck

This section documents intentional deviations from the original vgmck C implementation, primarily bug fixes.

### OPN2 (YM2612) Port 1 Address Calculation

**Original bug:** In `vgmck_opn2.c`, the address calculation for channels 4-6 (port 1) was incorrect:

```c
int ad=((assign[ch]&12)<<5)|(assign[ch]&3);
```

For `assign=4` (channel 0 on port 1), this produces `ad = 0x80`. When writing to frequency register `0xA4`:
- Address becomes `0x80 | 0xA4 = 0x124`
- Port selection: `(0x124 & 0x100) >> 8 = 1` (correct)
- Register: `0x124 & 0xFF = 0x24` (wrong - this is Timer A, not frequency!)

**Fix:** Changed shift from `<< 5` to `<< 6`:

```rust
let ad = (((self.assign[ch] as usize) & 12) << 6) | ((self.assign[ch] as usize) & 3);
```

For `assign=4`, this now produces `ad = 0x100`, so `0x100 | 0xA4 = 0x1A4`:
- Port selection: `(0x1A4 & 0x100) >> 8 = 1` (correct)
- Register: `0x1A4 & 0xFF = 0xA4` (correct)

This fix affects `src/chips/opn2.rs` in `update_oper` (line 56), `update_note` (line 108), and `send`/`send_with_macro_env` (lines 308, 317).

## License

GPL-3.0-or-later
