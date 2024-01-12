import "./Crypto.js";
import "./TextEncoder.js";
import { initSync, au_log_init } from "aucore/aucore";
import {
  ViewModel,
  PlayOperationVariantInput,
  PlayOperationOutput,
} from "typegen/types/au_types";
import { update, update_plain } from "./core";

export class RedSirenWorklet extends AudioWorkletProcessor {
  private vm: ViewModel["value"] | null = null;
  private initOutput?: any;

  constructor() {
    super();

    this.port.onmessage = this.onMessage.bind(this);
  }

  onRender = (vm: ViewModel) => {
    this.vm = vm.value;
  };

  onResolve = (output: Uint8Array) => {
    this.port.postMessage({
      type: "red-siren-resolve",
      output,
    });
  };

  onCapture = (output: Uint8Array) => {
    this.port.postMessage({
      type: "red-siren-capture",
      output,
    });
  };

  onMessage(msg: MessageEvent) {
    try {
      switch (msg.data.type) {
        case "send-wasm-module": {
          this.initOutput = initSync(msg.data.wasmBytes);
          console.info("wasm-ready");
          au_log_init();
          this.port.postMessage({
            type: "wasm-ready",
          });
          break;
        }
        case "red-siren-ev": {
          const ev = msg.data.ev as Uint8Array;
          console.info("event");
          update_plain(ev, this.onRender, this.onResolve, this.onCapture);
          break;
        }
        default:
          console.warn("unknown msg", msg);
          super.port.onmessage && super.port.onmessage(msg);
      }
    } catch (error) {
      console.error(error);
      this.port.postMessage({
        type: "error",
        error,
      });
    }
  }

  process(
    [[inputs]]: Float32Array[][],
    outputs: Float32Array[][],
    parameters: Record<string, Float32Array>
  ): boolean {
    if (!inputs) {
      console.warn("playing no input");
      return true;
    }

    update(
      new PlayOperationVariantInput([inputs] as unknown as number[][]),
      this.onRender,
      this.onResolve,
      this.onCapture
    );

    if (this.vm?.length) {
      for (let output of outputs) {
        for (let ch = 0; ch < output.length; ch++) {
          for (let s = 0; s < output[ch].length; s++) {
            if (this.vm[ch] !== undefined) {
              output[ch][s] = this.vm[ch][s];
            } else {
              output[ch][s] = this.vm[0][s];
            }
          }
        }
      }
    } else {
      console.log("playing no vm");
    }

    return true;
  }
}

registerProcessor("red-siren", RedSirenWorklet);
