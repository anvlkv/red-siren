import * as dat from "dat.gui";
import { ListTuple } from "shared_types/serde/types";
import { Config } from "shared_types/types/shared_types";
import { RedSirenNode } from "./lib";

interface IConfig
  extends Omit<
    Config,
    "serialize" | "deserialize" | "groups" | "buttons_group" | "channels"
  > {
  groups: number;
  buttons_group: number;
  channels: number;
}

const gui = new dat.GUI({ name: "worklet" });
gui.useLocalStorage = true;

const config: IConfig = {
  portrait: true,
  width: 420,
  height: 860,
  breadth: 100,
  length: 800,
  whitespace: 0,
  safe_area: [0, 30, 0, 30] as ListTuple<any>,
  groups: 2,
  buttons_group: 3,
  button_size: 78,
  button_track_margin: 0.2,
  sample_rate_hz: 44000,
  channels: 2,
  f0: 110,
};

const configFolder = gui.addFolder("Config");
const portrait = configFolder.add(config, "portrait");
const width = configFolder.add(config, "width");
const height = configFolder.add(config, "height");
const breadth = configFolder.add(config, "breadth");
const length = configFolder.add(config, "length");
const whitespace = configFolder.add(config, "whitespace");
const groups = configFolder.add(config, "groups");
const buttons_group = configFolder.add(config, "buttons_group");
const button_size = configFolder.add(config, "button_size");
const button_track_margin = configFolder.add(config, "button_track_margin");
const sample_rate_hz = configFolder.add(config, "sample_rate_hz");
const channels = configFolder.add(config, "channels");

const canvas = document.createElement("canvas");
canvas.setAttribute("style", "width: 100vw; height: 100vh;");
const root = document.getElementById("test")!;
root.appendChild(canvas);
const canvasCtx = canvas.getContext("2d")!;

const playback = { playing: false };

const playing = gui.add(playback, "playing");

let ctx: AudioContext | null = null;
let worklet: RedSirenNode | null = null;

let analyser1: AnalyserNode | null = null;
let analyser1Array: Uint8Array | null = null;
let analyser2: AnalyserNode | null = null;
let analyser2Array: Uint8Array | null = null;

const media = navigator.mediaDevices;

playing.onChange(async (playing) => {
  if (playing) {
    ctx = new AudioContext();

    const stream = await media.getUserMedia({ audio: true });

    const inputNode = new MediaStreamAudioSourceNode(ctx, {
      mediaStream: stream,
    });

    try {
      worklet = new RedSirenNode(ctx);
    } catch {
      await RedSirenNode.addModule(ctx);
      worklet = new RedSirenNode(ctx);
    }

    await worklet.init();

    worklet.port.postMessage({
      type: "red-siren-config",
      config: structuredClone(config),
    });

    analyser1 = ctx.createAnalyser();
    analyser1.fftSize = 2048;
    analyser1Array = new Uint8Array(analyser1.frequencyBinCount);
    analyser2 = ctx.createAnalyser();
    analyser2.fftSize = 2048;
    analyser2Array = new Uint8Array(analyser2.frequencyBinCount);

    inputNode
      .connect(analyser1)
      .connect(worklet)
      .connect(analyser2)
      .connect(ctx.destination);
  } else if (ctx) {
    await ctx.close();
    console.info("closed");

    ctx = null;
    worklet = null;
    analyser1 = null;
    analyser1Array = null;
    analyser2 = null;
    analyser2Array = null;
  }
});
[
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
  sample_rate_hz,
  channels,
].forEach((e) =>
  e.onChange(() => {
    worklet?.port.postMessage({
      type: "red-siren-config",
      config: structuredClone(config),
    });
  })
);

function draw(
  analyser: AnalyserNode,
  arr: Uint8Array,
  base: { x: number; y: number },
  size: { width: number; height: number }
) {
  analyser.getByteTimeDomainData(arr);

  canvasCtx.lineWidth = 2;
  canvasCtx.strokeStyle = "rgb(0, 200, 0)";
  canvasCtx.beginPath();

  const bufferLength = arr.length;

  const sliceWidth = (size.height * 1.0) / bufferLength;
  let y = base.y;

  for (let i = 0; i < bufferLength; i++) {
    const v = arr[i] / 128.0;
    const x = base.x + (v * size.width) / 2;

    if (i === 0) {
      canvasCtx.moveTo(x, y);
    } else {
      canvasCtx.lineTo(x, y);
    }

    y += sliceWidth;
  }

  canvasCtx.lineTo((base.x + size.width) / 2, base.y + size.height);
  canvasCtx.stroke();
}

function watcher() {
  const size = { width: canvas.width / 2, height: canvas.height };
  canvasCtx.clearRect(0, 0, canvas.width, canvas.height);
  canvasCtx.fillStyle = "rgb(10, 10, 10)";
  canvasCtx.fillRect(0, 0, canvas.width, canvas.height);

  if (analyser1 && analyser1Array) {
    draw(analyser1, analyser1Array, { x: 0, y: 0 }, size);
  }

  if (analyser2 && analyser2Array) {
    draw(analyser2, analyser2Array, { x: size.width, y: 0 }, size);
  }

  requestAnimationFrame(watcher);
}

requestAnimationFrame(watcher);
