import { process_event, view } from "shared/shared";
import {
  BincodeDeserializer,
  BincodeSerializer,
} from "shared_types/bincode/mod";
import type {
  Effect,
  Event
} from "shared_types/types/shared_types";
import {
  EffectVariantRender,
  Request,
  ViewModel
} from "shared_types/types/shared_types";

type CB = (vm: ViewModel) => void;

export function to_bin(event: Event,) {
  const serializer = new BincodeSerializer();
  event.serialize(serializer);
  const data = serializer.getBytes();
  return data;
}

export function update(
  event: Event,
  callback: CB
) {
  const serializer = new BincodeSerializer();
  event.serialize(serializer);
  const data = serializer.getBytes();
  const effects = process_event(data);

  const requests = deserializeRequests(effects);
  for (const { uuid, effect } of requests) {
    processEffect(uuid, effect, callback);
  }
}

export function update_plain(data: Uint8Array, callback: CB) {
  const effects = process_event(data);

  const requests = deserializeRequests(effects);
  for (const { uuid, effect } of requests) {
    processEffect(uuid, effect, callback);
  }
}

function processEffect(
  uuid: number[],
  effect: Effect,
  callback: CB
) {
  switch (effect.constructor) {
    case EffectVariantRender: {
      callback(deserializeView(view()));
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