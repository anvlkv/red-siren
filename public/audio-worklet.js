//! Audio Worklet Processor for Red Siren
//!
//! PURPOSE
//! -------
//! AudioWorkletProcessor implementation that bridges Web Audio API with
//! WASM-compiled DSP functions from the audio-system crate.
//!
//! ARCHITECTURE
//! ------------
//! - Runs in AudioWorklet thread (separate from main thread)
//! - Uses dynamic import to load WASM module in worklet context
//! - Calls WASM functions for real-time audio processing
//! - Communicates with main thread via MessagePort
//!
//! MAYA DRY KISS
//! -------------
//! - Focused on real-time audio processing only
//! - Simple message passing interface
//! - Clean error handling and logging

class RedSirenProcessor extends AudioWorkletProcessor {
  constructor(options) {
    super();

    // Processing state
    this.isInitialized = false;
    this.isProcessing = false;
    this.frameSize = 128; // Default Web Audio frame size
    this.sampleRate = sampleRate; // Global from AudioWorklet context
    this.channelCount = 2; // Stereo by default

    // WASM module
    this.wasmModule = null;

    // Buffer management - we'll allocate these as needed
    this.inputBufferPtr = null;
    this.outputBufferPtr = null;
    this.bufferSize = 0;

    // Message handling
    this.port.onmessage = (event) => {
      this.handleMessage(event.data);
    };

    // Initialize WASM module
    this.initializeWasm();

    this.log("RedSirenProcessor initialized");
  }

  log(message) {
    // Send log message to main thread
    this.port.postMessage({
      type: "log",
      payload: { message },
    });
  }

  sendAck(operation, success, errorMessage = null, id = null) {
    this.port.postMessage({
      type: "ack",
      id,
      payload: {
        operation,
        success,
        errorMessage,
      },
    });
  }

  async initializeWasm() {
    try {
      // Import WASM module in the worklet context
      const wasmModule = await import("./audio-worklet/audio_system.js");
      await wasmModule.default("./audio-worklet/audio_system_bg.wasm");
      this.wasmModule = wasmModule;

      this.log("WASM module loaded in processor context");

      // Initialize DSP with current sample rate and channel count
      const initialized = this.wasmModule.wasm_initialize_dsp(
        this.sampleRate,
        this.channelCount,
      );

      if (initialized) {
        this.isInitialized = true;
        this.log("WASM DSP initialized successfully");
      } else {
        this.log("WASM DSP initialization failed");
      }
    } catch (error) {
      this.log(`Failed to initialize WASM: ${error.message}`);
    }
  }

  handleMessage(data) {
    const { type, payload, id } = data;

    try {
      switch (type) {
        case "set_processing_state":
          this.isProcessing = payload.enabled;
          this.log(`Processing state changed: ${this.isProcessing}`);
          break;

        case "update_channel_count":
          this.channelCount = payload.channelCount;
          this.log(`Channel count updated: ${this.channelCount}`);
          break;

        case "start_dsp":
          this.handleStartDsp(payload, id);
          break;

        case "stop_dsp":
          this.handleStopDsp(id);
          break;

        case "pause_dsp":
          this.handlePauseDsp(id);
          break;

        case "resume_dsp":
          this.handleResumeDsp(id);
          break;

        case "rebuild_networks":
          this.handleRebuildNetworks(payload, id);
          break;

        case "set_activation_source":
          this.handleSetActivationSource(payload, id);
          break;

        case "set_band_control":
          this.handleSetBandControl(payload, id);
          break;

        case "get_snoop_data":
          this.handleGetSnoopData(payload, id);
          break;

        default:
          this.log(`Unknown message type: ${type}`);
      }
    } catch (error) {
      this.log(`Error handling message: ${error.message}`);
    }
  }

  handleStartDsp(payload, id) {
    if (!this.wasmModule || !this.isInitialized) {
      this.sendAck("start", false, "WASM module not initialized");
      return;
    }

    const started = this.wasmModule.wasm_start_dsp(
      payload.layoutJson,
      payload.configJson,
      payload.tunerConfigJson,
      payload.activationSource,
    );

    if (started) {
      this.isProcessing = true;
    }

    this.sendAck("start", started, started ? null : "Failed to start DSP", id);
  }

  handleStopDsp(id) {
    if (!this.wasmModule) {
      this.sendAck("stop", false, "WASM module not available");
      return;
    }

    const stopped = this.wasmModule.wasm_stop_dsp();

    if (stopped) {
      this.isProcessing = false;
    }

    this.sendAck("stop", stopped, stopped ? null : "Failed to stop DSP", id);
  }

  handlePauseDsp(id) {
    if (!this.wasmModule) {
      this.sendAck("pause", false, "WASM module not available");
      return;
    }

    const paused = this.wasmModule.wasm_pause_dsp();
    this.sendAck("pause", paused, paused ? null : "Failed to pause DSP", id);
  }

  handleResumeDsp(id) {
    if (!this.wasmModule) {
      this.sendAck("resume", false, "WASM module not available");
      return;
    }

    const resumed = this.wasmModule.wasm_resume_dsp();
    this.sendAck(
      "resume",
      resumed,
      resumed ? null : "Failed to resume DSP",
      id,
    );
  }

  handleRebuildNetworks(payload, id) {
    if (!this.wasmModule) {
      this.sendAck("rebuild_networks", false, "WASM module not available");
      return;
    }

    const rebuilt = this.wasmModule.wasm_rebuild_networks(
      payload.layoutJson,
      payload.configJson,
      payload.tunerConfigJson,
    );

    this.sendAck(
      "rebuild_networks",
      rebuilt,
      rebuilt ? null : "Failed to rebuild networks",
      id,
    );
  }

  handleSetActivationSource(payload, id) {
    if (!this.wasmModule) {
      this.sendAck("set_activation_source", false, "WASM module not available");
      return;
    }

    const sourceSet = this.wasmModule.wasm_set_activation_source(
      payload.source,
    );
    this.sendAck(
      "set_activation_source",
      sourceSet,
      sourceSet ? null : "Failed to set activation source",
      id,
    );
  }

  handleSetBandControl(payload, id) {
    if (!this.wasmModule) {
      this.sendAck("set_band_control", false, "WASM module not available");
      return;
    }

    const controlSet = this.wasmModule.wasm_set_band_control(
      payload.group,
      payload.key,
      payload.value,
    );

    this.sendAck(
      "set_band_control",
      controlSet,
      controlSet ? null : "Failed to set band control",
      id,
    );
  }

  handleGetSnoopData(payload, id) {
    if (!this.wasmModule) {
      this.port.postMessage({
        type: "snoop_response",
        id,
        payload: {
          snoopType: payload.snoopType,
          isSingleNode: payload.group !== null && payload.key !== null,
          singleNodeData: null,
          allNodesData: null,
          error: "WASM module not available",
        },
      });
      return;
    }

    try {
      let responsePayload;

      if (payload.group !== null && payload.key !== null) {
        // Single node request
        const samples =
          payload.snoopType === "output"
            ? this.wasmModule.wasm_get_output_snoop(payload.group, payload.key)
            : this.wasmModule.wasm_get_activation_snoop(
                payload.group,
                payload.key,
              );

        responsePayload = {
          snoopType: payload.snoopType,
          isSingleNode: true,
          singleNodeData: {
            group: payload.group,
            key: payload.key,
            samples: Array.from(samples),
          },
          allNodesData: null,
        };
      } else {
        // All nodes request
        const allSnoopsJson =
          payload.snoopType === "output"
            ? this.wasmModule.wasm_get_all_output_snoops()
            : this.wasmModule.wasm_get_all_activation_snoops();

        responsePayload = {
          snoopType: payload.snoopType,
          isSingleNode: false,
          singleNodeData: null,
          allNodesData: JSON.parse(allSnoopsJson),
        };
      }

      this.port.postMessage({
        type: "snoop_response",
        id,
        payload: responsePayload,
      });
    } catch (error) {
      this.port.postMessage({
        type: "snoop_response",
        id,
        payload: {
          snoopType: payload.snoopType,
          isSingleNode: payload.group !== null && payload.key !== null,
          singleNodeData: null,
          allNodesData: null,
          error: error.message,
        },
      });
    }
  }

  process(inputs, outputs, parameters) {
    // Early return if not ready for processing
    if (!this.isInitialized || !this.wasmModule || !this.isProcessing) {
      // Fill outputs with silence
      for (let outputIndex = 0; outputIndex < outputs.length; outputIndex++) {
        const output = outputs[outputIndex];
        for (
          let channelIndex = 0;
          channelIndex < output.length;
          channelIndex++
        ) {
          output[channelIndex].fill(0);
        }
      }
      return true;
    }

    try {
      const input = inputs[0];
      const output = outputs[0];

      if (!input || !output || input.length === 0 || output.length === 0) {
        return true;
      }

      const frameCount = input[0].length;
      const actualChannelCount = Math.min(
        input.length,
        output.length,
        this.channelCount,
      );

      // Allocate temporary buffers for interleaved audio
      const requiredBufferSize = frameCount * actualChannelCount;

      if (!this.inputBuffer || this.inputBuffer.length < requiredBufferSize) {
        this.inputBuffer = new Float32Array(requiredBufferSize);
        this.outputBuffer = new Float32Array(requiredBufferSize);
      }

      // Interleave input samples into buffer
      let bufferIndex = 0;
      for (let frame = 0; frame < frameCount; frame++) {
        for (let channel = 0; channel < actualChannelCount; channel++) {
          this.inputBuffer[bufferIndex] = input[channel]
            ? input[channel][frame]
            : 0;
          bufferIndex++;
        }
      }

      // Call WASM processing function with raw pointers
      // Note: We need to get the actual memory pointers from the WASM module
      const inputPtr = this.wasmModule.memory.buffer.byteLength; // This is a placeholder - proper implementation needs memory management
      const outputPtr = this.wasmModule.memory.buffer.byteLength; // This is a placeholder - proper implementation needs memory management

      // For now, we'll use a simplified approach and copy data through the arrays
      // A proper implementation would need to allocate memory in WASM and pass pointers
      const processed = this.wasmModule.wasm_process_audio(
        0, // inputPtr placeholder
        0, // outputPtr placeholder
        frameCount,
      );

      if (!processed) {
        // WASM processing failed, fill with silence
        this.outputBuffer.fill(0);
      }

      // For testing purposes, let's generate some simple output
      // This should be replaced with actual WASM output once memory management is fixed
      for (let i = 0; i < this.outputBuffer.length; i++) {
        // Simple sine wave for testing
        this.outputBuffer[i] =
          Math.sin(
            ((Date.now() * 0.001 + i * 0.1) * 2 * Math.PI * 440) /
              this.sampleRate,
          ) * 0.1;
      }

      // Deinterleave output buffer back to Web Audio format
      bufferIndex = 0;
      for (let frame = 0; frame < frameCount; frame++) {
        for (let channel = 0; channel < actualChannelCount; channel++) {
          if (output[channel]) {
            output[channel][frame] = this.outputBuffer[bufferIndex];
          }
          bufferIndex++;
        }
      }
    } catch (error) {
      this.log(`Processing error: ${error.message}`);

      // Fill outputs with silence on error
      for (let outputIndex = 0; outputIndex < outputs.length; outputIndex++) {
        const output = outputs[outputIndex];
        for (
          let channelIndex = 0;
          channelIndex < output.length;
          channelIndex++
        ) {
          output[channelIndex].fill(0);
        }
      }
    }

    return true;
  }
}

// Register the processor
registerProcessor("red-siren-processor", RedSirenProcessor);
