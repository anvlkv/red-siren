import "./Crypto.js";
import "./TextEncoder.js";
import { initSync, au_log_init } from "aucore/aucore";
import {
  ViewModel,
  PlayOperationVariantInput,
} from "typegen/types/au_types";
import { update, update_plain } from "./core";

export class RedSirenWorklet extends AudioWorkletProcessor {
  private vm: ViewModel["value"][] = [];
  private initOutput?: any;
  private fillBuffer = true;

  constructor() {
    super();

    this.port.onmessage = this.onMessage.bind(this);
  }

  onRender = (vm: ViewModel) => {
    this.vm.push(vm.value);
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
        case "clear-buffer" : {
          this.vm = [];
          this.fillBuffer = true;
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

    if (this.vm.length >= 42) {
      this.fillBuffer = false;
    }

    if (this.vm.length && !this.fillBuffer) {
      const buffer = this.vm.splice(0, 1)[0];
      for (let output of outputs) {
        for (let ch = 0; ch < output.length; ch++) {
          for (let s = 0; s < output[ch].length; s++) {
            if (buffer[ch] !== undefined) {
              output[ch][s] = buffer[ch][s];
            } else {
              output[ch][s] = buffer[0][s];
            }
          }
        }
      }
    }

    return true;
  }
}

registerProcessor("red-siren", RedSirenWorklet);
