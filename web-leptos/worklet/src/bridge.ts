import {
  PlayOperation,
  PlayOperationVariantPermissions,
  PlayOperationOutputVariantPermission,
  PlayOperationOutputVariantSuccess,
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

  private sender?: (data: Uint8Array) => void;

  onResolve = (out: Uint8Array) => {
    try {
      this.sender!(out);
    } catch (e) {
      console.error(e);
      this.ctx?.suspend().then(() => {
        console.log("suspended");
      });
    }
  };

  public request(bytes: Uint8Array, sender: (data: Uint8Array) => void) {
    const op = PlayOperation.deserialize(new BincodeDeserializer(bytes));
    const ser = new BincodeSerializer();

    this.sender = sender;

    if (!this.ctx || !this.redSirenNode || !this.inputNode) {
      switch (op.constructor) {
        case PlayOperationVariantPermissions: {
          (async () => {
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
              new PlayOperationOutputVariantPermission(true).serialize(ser);
            } catch (e) {
              console.error(e);
              new PlayOperationOutputVariantPermission(false).serialize(ser);
            }
            this.onResolve(ser.getBytes());
          })();
          break;
        }
        case PlayOperationVariantInstallAU: {
          (async () => {
            try {
              await RedSirenNode.addModule(this.ctx!);
              this.redSirenNode = new RedSirenNode(this.ctx!);
              console.log("init worklet");
              await this.redSirenNode!.init();
              this.redSirenNode.onResolve = this.onResolve;
              this.inputNode!.connect(this.redSirenNode!).connect(
                this.ctx!.destination
              );
              
              await this.ctx.suspend();

              new PlayOperationOutputVariantSuccess(true).serialize(ser);
            } catch (e) {
              console.error(e);
              new PlayOperationOutputVariantSuccess(false).serialize(ser);
            }
            this.onResolve(ser.getBytes());
          })();
          break;
        }
        default: {
          throw new Error("init before requesting capabilities");
        }
      }
    } else {
      switch (op.constructor) {
        case PlayOperationVariantResume: {
          (async () => {
            try {
              await this.ctx.resume();
              console.log("resumed");
              new PlayOperationOutputVariantSuccess(true).serialize(ser);
            } catch (e) {
              console.error(e);
              new PlayOperationOutputVariantSuccess(false).serialize(ser);
            }
            this.onResolve(ser.getBytes());
          })();
          break;
        }
        case PlayOperationVariantSuspend: {
          (async () => {
            try {
              await this.ctx.suspend();
              console.log("suspended");
            } catch (e) {
              console.error(e);
            }
          })();
          break;
        }
        default: {
          console.log("forwarding");
          try {
            this.redSirenNode.forward(bytes);
          } catch (e) {
            console.error(e);
            new PlayOperationOutputVariantSuccess(false).serialize(ser);
            this.onResolve(ser.getBytes());
          }
        }
      }
    }
  }
}
