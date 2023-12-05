package com.anvlkv.redsiren.app

class Playback {
    init
    {
        System.loadLibrary("rs-audio-lib");
    }
    private external fun startEngine()

    private external fun stopEngine()
}