// SoundEngine — port of SoundEngine.swift.
// The 28 WAVs are the macOS app's own files (see SOUNDS_DIR in vite.config.ts);
// they are served at /sounds/<name>.wav. Default volume 0.12, slider range 0–0.2,
// exactly like the Mac player, and several sounds may overlap.

export const SOUND_NAMES = [
  "peek", "open", "close", "hover", "blip", "slap", "annoyed", "dizzy", "greet",
  "work", "finish", "error", "approval", "question", "approve", "gulp", "tick",
  "send", "love", "pop", "proud", "wink", "yawn", "attach", "think", "search",
  "rate", "sleep",
] as const;

export type SoundName = (typeof SOUND_NAMES)[number];

class SoundEngine {
  enabled = true;
  volume = 0.5;

  private ctx: AudioContext | null = null;
  private master: GainNode | null = null;
  private buffers = new Map<string, AudioBuffer>();
  private loading: Promise<void> | null = null;
  private idleTimer: number | null = null;

  /** Creates the context and decodes every WAV. Safe to call more than once. */
  preload(): Promise<void> {
    if (this.loading) return this.loading;
    this.loading = (async () => {
      const Ctor = window.AudioContext ?? (window as unknown as { webkitAudioContext: typeof AudioContext }).webkitAudioContext;
      if (!Ctor) return;
      const ctx = new Ctor();
      this.ctx = ctx;
      const master = ctx.createGain();
      master.gain.value = this.gainFor(this.volume);
      // The limiter is what lets the slider go past unity without turning into
      // noise — but only if it is set right. Web Audio caps `threshold` at
      // 0 dBFS, so a limiter parked at -3 dB does not merely guard the peaks,
      // it *quietens the whole sound*: typical material sits at -13 dBFS, lands
      // above that threshold, and gets pulled down. That is why "max" felt weak
      // even at gain 4. At 0 dB it only touches what is actually over full scale.
      const limiter = ctx.createDynamicsCompressor();
      limiter.threshold.value = 0;
      limiter.knee.value = 0;
      limiter.ratio.value = 20;
      limiter.attack.value = 0.002;
      limiter.release.value = 0.08;
      master.connect(limiter);
      limiter.connect(ctx.destination);
      this.master = master;
      await Promise.all(
        SOUND_NAMES.map(async (name) => {
          try {
            const res = await fetch(`/sounds/${name}.wav`);
            if (!res.ok) return;
            const buf = await ctx.decodeAudioData(await res.arrayBuffer());
            this.buffers.set(name, buf);
          } catch {
            /* a missing sound must never break the island */
          }
        }),
      );
    })();
    return this.loading;
  }

  /** WebView2 can hand us a suspended context; call after any user input. */
  resume() {
    if (this.idleTimer != null) {
      window.clearTimeout(this.idleTimer);
      this.idleTimer = null;
    }
    void this.ctx?.resume();
  }

  /**
   * Called when the island goes quiet. A running AudioContext keeps an audio
   * thread and its render quantum alive even with nothing playing, which shows
   * up as a steady trickle of CPU on a machine that is supposed to be idle.
   *
   * The delay covers the tail of whatever just played — suspending mid-sound
   * would clip it — and `play()` resumes the context on its own.
   */
  idle() {
    if (!this.ctx || this.ctx.state !== "running" || this.idleTimer != null) return;
    this.idleTimer = window.setTimeout(() => {
      this.idleTimer = null;
      void this.ctx?.suspend();
    }, 1500);
  }

  setVolume(v: number) {
    // SPEC §9 pins the *Mac* player at 0–0.2 and a 0.12 default. Those numbers
    // only made sense against AVAudioPlayer and WAVs rendered at gain ×6; on
    // Linux they put every sound at a fifth of the mixer's own ceiling, which is
    // why "max" was inaudible. The Linux scale is this one, and docs/LINUX.md
    // records the deviation.
    this.volume = Math.max(0, Math.min(1, v));
    if (this.master) this.master.gain.value = this.gainFor(this.volume);
  }

  /**
   * Slider 0–1 → gain, and gain goes well past 1.
   *
   * A Web Audio gain of 1 means "a full-scale source", which is still only as
   * loud as the system mixer allows. Measured on this machine: the 28 sounds
   * peak at -13 dBFS on average, so a gain of 1 would play them a seventh of
   * full volume — the "max" that was inaudible across a desk.
   *
   * The gain tops out at 12 (+22 dB), which puts a typical sound right at full
   * scale and lets the limiter hold the peaks there instead of clipping them.
   * That is the physical ceiling for this path: PulseAudio will not amplify
   * past 100 % unless the session carries the `overamplification` flag, which
   * is a GNOME setting Cinnamon does not offer, so nothing after the gain can
   * make the sound louder than a full-scale sample. Past this point only the
   * system volume can help.
   *
   * The curve is not linear: the quiet half of the slider should stay quiet, so
   * it is where the exponential does its work.
   */
  private gainFor(v: number): number {
    return Math.pow(v, 1.6) * 12;
  }

  setEnabled(on: boolean) {
    this.enabled = on;
  }

  play(name: SoundName | string) {
    if (!this.enabled) return;
    const ctx = this.ctx;
    const master = this.master;
    const buf = this.buffers.get(name);
    if (!ctx || !master || !buf) return;
    if (this.idleTimer != null) {
      window.clearTimeout(this.idleTimer);
      this.idleTimer = null;
    }
    if (ctx.state === "suspended") void ctx.resume();
    const src = ctx.createBufferSource();
    src.buffer = buf;
    src.connect(master);
    src.start();
  }
}

export const Sound = new SoundEngine();
