import {
  PlayOperation,
  PlayOperationVariantPermissions,
  PlayOperationOutputVariantPermission,
  PlayOperationOutputVariantSuccess,
  PlayOperationVariantInstallAU,
  PlayOperationVariantResume,
  PlayOperationVariantSuspend,
} from "shared_types/types/au_types";
import { RedSirenNode } from "./node";
import {
  BincodeDeserializer,
  BincodeSerializer,
} from "shared_types/bincode/mod";

export class PlaybackBridge {
  private ctx?: AudioContext;
  private worklet?: RedSirenNode;
  private inputNode?: MediaStreamAudioSourceNode;
  private registry = new Map<
    string,
    [(data: Uint8Array) => void, (reason: any) => void]
  >();

  // #[wasm_bindgen(structural, method, getter, js_name = callHost, js_class = "PlaybackBridge")]
  // pub fn call_host_block(this: &PlaybackJs) -> Option<::js_sys::Function>;

  // #[wasm_bindgen(structural, method, setter, js_name = callHost, js_class = "PlaybackBridge")]
  // pub fn set_call_host_block(this: &PlaybackJs, val: Option<&::js_sys::Function>);
  public callHost?: any;

  constructor() {}

  onResolve = (out: Uint8Array, id?: string) => {
    if (id) {
      this.registry.get(id)![0](out);
      this.registry.delete(id);
    } else if (this.callHost) {
      this.callHost(out);
    } else {
      console.log(out);
    }
  };

  // #[wasm_bindgen(method, js_class = "PlaybackBridge")]
  // pub fn request(this: &PlaybackJs, req: &JsValue) -> Promise;
  public async request(bytes: Uint8Array): Promise<Uint8Array> {
    const op = PlayOperation.deserialize(new BincodeDeserializer(bytes));
    const ser = new BincodeSerializer();

    if (!this.ctx || !this.worklet || !this.inputNode) {
      switch (op.constructor) {
        case PlayOperationVariantPermissions: {
          try {
            const media = navigator.mediaDevices;
            const stream = await media.getUserMedia({ audio: true });
            const ctx = new AudioContext();
            const inputNode = new MediaStreamAudioSourceNode(ctx, {
              mediaStream: stream,
            });
            this.ctx = ctx;
            this.inputNode = inputNode;

            new PlayOperationOutputVariantPermission(true).serialize(ser);
          } catch (e) {
            console.error(e);
            new PlayOperationOutputVariantPermission(false).serialize(ser);
          }
          return ser.getBytes();
        }
        case PlayOperationVariantInstallAU: {
          try {
            await RedSirenNode.addModule(this.ctx!);
            this.worklet = new RedSirenNode(this.ctx!);
            console.log("init worklet");
            await this.worklet!.init();
            
            this.inputNode!.connect(this.worklet!).connect(
              this.ctx!.destination
            );
            this.worklet.onResolve = this.onResolve;

            new PlayOperationOutputVariantSuccess(true).serialize(ser);
          } catch (e) {
            console.error(e);
            new PlayOperationOutputVariantSuccess(false).serialize(ser);
          }
          return ser.getBytes();
        }
        default: {
          throw new Error("init before requesting capabilities");
        }
      }
    } else {
      switch (op.constructor) {
        case PlayOperationVariantResume: {
          try {
            await this.ctx.resume();

            new PlayOperationOutputVariantSuccess(true).serialize(ser);
          } catch (e) {
            console.error(e);
            new PlayOperationOutputVariantSuccess(false).serialize(ser);
          }
          return ser.getBytes();
        }
        case PlayOperationVariantSuspend: {
          try {
            await this.ctx.suspend();

            new PlayOperationOutputVariantSuccess(true).serialize(ser);
          } catch (e) {
            console.error(e);
            new PlayOperationOutputVariantSuccess(false).serialize(ser);
          }
          return ser.getBytes();
        }
        default: {
          console.log("forwarding");
          try {
            const crypto = window.crypto;
            const id = crypto.randomUUID();
            return new Promise((resolve, reject) => {
              this.registry.set(id, [resolve, reject]);
              this.worklet.forwardWithId(bytes, id);
            });
          } catch (e) {
            console.error(e);
            new PlayOperationOutputVariantSuccess(false).serialize(ser);
            return ser.getBytes();
          }
        }
      }
    }
  }
}
