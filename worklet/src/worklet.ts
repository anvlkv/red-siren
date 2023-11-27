import "./TextEncoder.js";
import "./Crypto.js";
import { update } from "./core";
import { initSync } from "shared/shared";
import {
  ActivityVariantPlay,
  Config,
  EventVariantActivate,
  EventVariantConfigureApp,
  EventVariantInstrumentEvent,
  EventVariantStart,
  InstrumentEV,
  InstrumentEVVariantPlayback,
  PlaybackEV,
  PlaybackEVVariantDataIn,
  PlaybackEVVariantPlay,
  ViewModel,
} from "shared_types/types/shared_types";

function toWindows<T>(inputArray: T[], size: number) {
  return Array.from(
    { length: inputArray.length - (size - 1) }, //get the appropriate length
    (_, index) => inputArray.slice(index, index + size) //create the windows
  );
}

export class RedSirenWorklet extends AudioWorkletProcessor {
  private vm: ViewModel | null = null;
  private core: any;
  constructor() {
    super();

    this.port.onmessage = this.onMessage.bind(this);
  }

  onRender = (vm: ViewModel) => {
    this.vm = vm;
  };

  onMessage(msg: MessageEvent) {
    switch (msg.data.type) {
      case "send-wasm-module":
        try {
          this.core = initSync(msg.data.wasmBytes);
          update(new EventVariantStart(), this.onRender);
          this.port.postMessage({
            type: "wasm-ready",
          });
        } catch (error) {
          this.port.postMessage({
            type: "error",
            error,
          });
        }
        break;
      case "red-siren-config":
        try {
          const {
            portrait,
            width,
            height,
            whitespace,
            breadth,
            button_size,
            button_track_margin,
            buttons_group,
            channels,
            f0,
            groups,
            length,
            safe_area,
            sample_rate_hz,
          } = msg.data.config as Config;

          update(
            new EventVariantConfigureApp(
              new Config(
                portrait,
                width,
                height,
                breadth,
                length,
                whitespace,
                groups,
                buttons_group,
                button_size,
                button_track_margin,
                safe_area,
                f0,
                sample_rate_hz,
                channels
              )
            ),
            this.onRender
          );
          update(
            new EventVariantActivate(new ActivityVariantPlay()),
            this.onRender
          );
          update(
            new EventVariantInstrumentEvent(
              new InstrumentEVVariantPlayback(new PlaybackEVVariantPlay(true))
            ),
            this.onRender
          );
        } catch (error) {
          this.port.postMessage({
            type: "error",
            error,
          });
        }
        break;
      default:
        super.port.onmessage && super.port.onmessage(msg);
    }
  }

  process(
    [inputs]: Float32Array[][],
    outputs: Float32Array[][],
    parameters: Record<string, Float32Array>
  ): boolean {
    if (!inputs || !this.vm) {
      return true;
    }

    update(
      new EventVariantInstrumentEvent(
        new InstrumentEVVariantPlayback(new PlaybackEVVariantDataIn(inputs as any))
      ),
      this.onRender
    );

    if (this.vm.instrument.audio_data.length) {
      for (let output of outputs) {
        for (let ch = 0; ch < output.length; ch++) {
          for (let s = 0; s < output[ch].length; s++) {
            output[ch][s] = this.vm.instrument.audio_data[ch][s];
          }
        }
      }
    }

    return true;
  }
}

registerProcessor("red-siren", RedSirenWorklet);
