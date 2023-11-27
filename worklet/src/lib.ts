export default class RedSirenNode extends AudioWorkletNode {
  public static workletUrl = new URL('./worklet.ts', import.meta.url);
  private static wasmUrl = new URL('../node_modules/shared/shared_bg.wasm', import.meta.url)

  
  public async init() {
    const response = await window.fetch(RedSirenNode.wasmUrl);
    const wasmBytes = await response.arrayBuffer();

    this.port.onmessage = this.onmessage.bind(this);

    this.port.postMessage({
      type: "send-wasm-module",
      wasmBytes,
    });
  }

  onprocessorerror = (err: Event) => {
    console.log(
      `An error from AudioWorkletProcessor.process() occurred: ${err}`
    );
  };

  onmessage = (msg: MessageEvent) => {
    console.log(msg);
  }
}