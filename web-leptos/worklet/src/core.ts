import { au_process_event, au_view } from "aucore/aucore";
import { BincodeDeserializer, BincodeSerializer } from "typegen/bincode/mod";
import type { Effect, PlayOperation } from "typegen/types/au_types";
import {
  EffectVariantCapture,
  EffectVariantRender,
  EffectVariantResolve,
  Request,
  ViewModel,
} from "typegen/types/au_types";

type RenderCB = (vm: ViewModel) => void;
type ResolveCB = (output: Uint8Array) => void;
type CaptureCB = (output: Uint8Array) => void;

export function to_bin(event: PlayOperation) {
  const serializer = new BincodeSerializer();
  event.serialize(serializer);
  const data = serializer.getBytes();
  return data;
}

export function update(
  event: PlayOperation,
  callback: RenderCB,
  resolve: ResolveCB,
  capture: CaptureCB
) {
  const serializer = new BincodeSerializer();
  event.serialize(serializer);
  const data = serializer.getBytes();
  const effects = au_process_event(data);

  const requests = deserializeRequests(effects);
  for (const { uuid, effect } of requests) {
    processEffect(uuid, effect, callback, resolve, capture);
  }
}

export function update_plain(
  data: Uint8Array,
  callback: RenderCB,
  resolve: ResolveCB,
  capture: CaptureCB
) {
  const effects = au_process_event(data);

  const requests = deserializeRequests(effects);
  for (const { uuid, effect } of requests) {
    processEffect(uuid, effect, callback, resolve, capture);
  }
}

function processEffect(
  uuid: number[],
  effect: Effect,
  callback: RenderCB,
  resolve: ResolveCB,
  capture: CaptureCB
) {
  switch (effect.constructor) {
    case EffectVariantRender: {
      callback(deserializeView(au_view()));
      break;
    }
    case EffectVariantResolve: {
      const value = (effect as EffectVariantResolve).value;
      const serializer = new BincodeSerializer();
      value.serialize(serializer);
      const data = serializer.getBytes();
      resolve(data);
      break;
    }
    case EffectVariantCapture: {
      const value = (effect as EffectVariantCapture).value;
      const serializer = new BincodeSerializer();
      value.serialize(serializer);
      const data = serializer.getBytes();
      capture(data);
      break;
    }
    default:
      break;
  }
}

function deserializeRequests(bytes: Uint8Array) {
  const deserializer = new BincodeDeserializer(bytes);
  const len = deserializer.deserializeLen();
  const requests: Request[] = [];
  for (let i = 0; i < len; i++) {
    const request = Request.deserialize(deserializer);
    requests.push(request);
  }
  return requests;
}

function deserializeView(bytes: Uint8Array) {
  return ViewModel.deserialize(new BincodeDeserializer(bytes));
}
