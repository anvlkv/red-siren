import {
  PlayOperation,
  PlayOperationVariantPermissions,
  PlayOperationOutputVariantSuccess,
  PlayOperationOutputVariantFailure,
  PlayOperationVariantInstallAU,
  PlayOperationVariantResume,
  PlayOperationVariantSuspend,
} from "typegen/types/au_types";
import { RedSirenNode } from "./node";
import { BincodeDeserializer, BincodeSerializer } from "typegen/bincode/mod";

export class PlaybackBridge {
  private ctx?: AudioContext;
  private redSirenNode?: RedSirenNode;
  private inputNode?: MediaStreamAudioSourceNode;

  public on_capture?: (data: Uint8Array) => void;

  private current_resolve = (out: Uint8Array) => {};
  onResolve = (out: Uint8Array) => {
    this.current_resolve(out);
  };

  public async request(bytes: Uint8Array): Promise<Uint8Array> {
    const op = PlayOperation.deserialize(new BincodeDeserializer(bytes));
    const ser = new BincodeSerializer();

    if (!this.ctx || !this.redSirenNode || !this.inputNode) {
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

            console.log("permissions");
            new PlayOperationOutputVariantSuccess().serialize(ser);
          } catch (e) {
            console.error(e);
            new PlayOperationOutputVariantFailure().serialize(ser);
          }
          return ser.getBytes();
        }
        case PlayOperationVariantInstallAU: {
          try {
            await RedSirenNode.addModule(this.ctx!);
            this.redSirenNode = new RedSirenNode(this.ctx!);
            console.log("init worklet");
            await this.redSirenNode!.init();
            this.redSirenNode.onResolve = this.onResolve;
            this.redSirenNode.onCapture = this.on_capture!;
            this.inputNode!.connect(this.redSirenNode!).connect(
              this.ctx!.destination
            );

            await this.ctx.suspend();

            new PlayOperationOutputVariantSuccess().serialize(ser);
          } catch (e) {
            console.error(e);
            new PlayOperationOutputVariantFailure().serialize(ser);
          }
          return ser.getBytes();
        }
        default: {
          console.error("unprepared for:", op.constructor);
          throw new Error("init before requesting capabilities");
        }
      }
    } else {
      switch (op.constructor) {
        case PlayOperationVariantResume: {
          try {
            await this.ctx.resume();
            this.redSirenNode?.clearBuffer();
            console.log("resumed");
            new PlayOperationOutputVariantSuccess().serialize(ser);
          } catch (e) {
            console.error(e);
            new PlayOperationOutputVariantFailure().serialize(ser);
          }
          return ser.getBytes();
        }
        case PlayOperationVariantSuspend: {
          try {
            await this.ctx.suspend();
            console.log("suspended");
            new PlayOperationOutputVariantSuccess().serialize(ser);
          } catch (e) {
            console.error(e);
            new PlayOperationOutputVariantFailure().serialize(ser);
          }
          return ser.getBytes();
        }
        default: {
          console.log("forwarding");
          try {
            return new Promise((resolve, reject) => {
              this.current_resolve = resolve;
              this.redSirenNode.forward(bytes);
            });
          } catch (e) {
            console.error(e);
            new PlayOperationOutputVariantFailure().serialize(ser);
            return ser.getBytes();
          }
        }
      }
    }
  }
}
