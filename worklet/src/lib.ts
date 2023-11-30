import workletUrl from "./worklet?worker&url";
import wasmUrl from "shared/shared_bg.wasm?url";

export class RedSirenNode extends AudioWorkletNode {
  public static workletUrl = workletUrl;
  private static wasmUrl = wasmUrl;

  private initPromiseResolve: (() => void) | null = null;
  private initPromiseReject: ((reason: any) => void) | null = null;

  constructor(ctx: AudioContext) {
    super(ctx, "red-siren");
  }

  public init() {
    return new Promise<void>(async (resolve, reject) => {
      const response = await window.fetch(RedSirenNode.wasmUrl);
      const wasmBytes = await response.arrayBuffer();
  
      this.port.onmessage = this.onmessage.bind(this);
  
      this.port.postMessage({
        type: "send-wasm-module",
        wasmBytes,
      });
      this.initPromiseResolve = resolve;
      this.initPromiseReject = reject;
    })
  }

  public static async addModule(ctx: AudioContext) {
    try {
      new AudioWorkletNode(ctx, "red-siren")
    }
    catch {
      await ctx.audioWorklet.addModule(RedSirenNode.workletUrl)
    }
  }

  onprocessorerror = (err: Event) => {
    console.error(
      `An error from AudioWorkletProcessor: ${err}`
    );

    if (this.initPromiseReject) {
      this.initPromiseReject(err);
      this.initPromiseReject = null;
      this.initPromiseResolve = null;
    }
  };

  onmessage = (msg: MessageEvent) => {
    if (msg.data.type === "wasm-ready") {
      this.initPromiseResolve && this.initPromiseResolve();
    }
    else if (msg.data.type === "error") {
      this.onprocessorerror(msg.data.error);
    }
  };

  public forward(ev: Uint8Array) {
    this.port.postMessage({
      type: "red-siren-ev",
      ev,
    });
  }
}
