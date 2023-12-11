//
//  playback.swift
//  RedSiren
//
//  Created by a.nvlkv on 01/12/2023.
//

import Foundation
import AVFoundation
import AVFAudio
import SharedTypes
import UIKit
import SwiftUI





class Playback: NSObject, ObservableObject {
    let type: String = "aufx"
    let subType: String = "rsau"
    let manufacturer: String = "nvlk"
    
    private var session: AVAudioSession?
    private var audioEngine: AVAudioEngine?
    private var avAudioUnit: AVAudioUnit?
    private var evChannel: AUMessageChannel?
    
    let config: Config
    let ev: (InstrumentEV) -> Void
    
    init(config: Config, ev: @escaping (InstrumentEV) -> Void) {
        self.config=config
        self.ev = ev
    }
    
//    let redSirenNode = AudioUnit(){_, _, frameCount, outputBusNumber, outputData, pullInputBlock in
//    }
    
    public func setup(completion: @escaping (Result<Bool, Error>) -> Void) {
        Task.init{
            setupAudioSession()
            if await setupAudioEngine() {
                guard let component = AVAudioUnit.findComponent(type: type, subType: subType, manufacturer: manufacturer) else {
                    fatalError("Failed to find component with type: \(type), subtype: \(subType), manufacturer: \(manufacturer))" )
                }
                AVAudioUnit.instantiate(with: component.audioComponentDescription,
                                        options: AudioComponentInstantiationOptions.loadOutOfProcess) { avAudioUnit, error in
                    guard let audioUnit = avAudioUnit, error == nil else {
                        completion(.failure(error!))
                        return
                    }
                    
                    self.avAudioUnit = audioUnit

                    self.setupNodes{completion_setup in
                        completion(completion_setup)
                    }
                }
            }
            else {
                completion(.success(false))
            }
        }
    }
    
    func setupNodes(completion_setup: @escaping (Result<Bool, Error>) -> Void) {
        let engine = self.audioEngine!
        let audioUnit = self.avAudioUnit!
//        let format = AVAudioFormat.init(commonFormat: .pcmFormatFloat32, sampleRate: config.sample_rate_hz, channels: .init(config.channels), interleaved: true)
//        let input = engine.inputNode
//        do {
//            try input.setVoiceProcessingEnabled(true)
//        }
//        catch {
//            completion_setup(.failure(error))
//        }
//        let mixer = engine.mainMixerNode
//        engine.attach(audioUnit)
//
//        engine.connect(input, to: audioUnit, format: format)
//        engine.connect(audioUnit, to: mixer, format: format)
//        let hardwareFormat = engine.outputNode.outputFormat(forBus: 0)
//        engine.connect(engine.mainMixerNode, to: engine.outputNode, format: hardwareFormat)
//
//
//        engine.prepare()
//
//        do {
//            try engine.start()
//
////           self.evChannel = audioUnit.auAudioUnit.messageChannel(for: "rsev")
////           let channel = self.evChannel!
////           let configEv = Event.instrumentEvent(InstrumentEV.createWithConfig(self.config))
////           let data = try configEv.bincodeSerialize()
////           _ = channel.callAudioUnit!(["ev": data])
//
//            completion_setup(.success(true))
//        }
//        catch {
//            completion_setup(.failure(error))
//        }
    }

    
    var isAuthorized: Bool {
        get async {
            let status = AVCaptureDevice.authorizationStatus(for: .audio)
            
            // Determine if the user previously authorized camera access.
            var isAuthorized = status == .authorized
            
            // If the system hasn't determined the user's authorization status,
            // explicitly prompt them for approval.
            if status == .notDetermined {
                isAuthorized = await AVCaptureDevice.requestAccess(for: .audio)
            }
            
            return isAuthorized
        }
    }
    
    func setUpCaptureSession() async -> Bool {
        guard await isAuthorized else { return false }
        // Set up the capture session.
        
        do {
            try session!.setActive(true)
            return true
        }
        catch {
            return false
        }
    }
    
    public func evPort(event: Event) {
        
    }
    
    func setupAudioSession() {
        self.session = AVAudioSession.sharedInstance()

        do {
            try session!.setCategory(.playAndRecord, mode: .measurement, options: .defaultToSpeaker)
        } catch {
            print("Could not set the audio category: \(error.localizedDescription)")
        }

//        do {
//            try session!.setPreferredSampleRate(self.config.sample_rate_hz)
//        } catch {
//            print("Could not set the preferred sample rate: \(error.localizedDescription)")
//        }
    }
    
    func setupAudioEngine() async -> Bool {
        audioEngine = AVAudioEngine()
        return await setUpCaptureSession()
    }
    
    @objc
    func handleInterruption(_ notification: Notification) {
        guard let userInfo = notification.userInfo,
            let typeValue = userInfo[AVAudioSessionInterruptionTypeKey] as? UInt,
            let type = AVAudioSession.InterruptionType(rawValue: typeValue) else { return }
        
        switch type {
        case .began:
            audioEngine?.pause()
            break
        case .ended:
            if let audioEngine {
                try! audioEngine.start()
            }
            break
        @unknown default:
            fatalError("Unknown type: \(type)")
        }
    }
    
    @objc
    func handleRouteChange(_ notification: Notification) {
        guard let userInfo = notification.userInfo,
            let reasonValue = userInfo[AVAudioSessionRouteChangeReasonKey] as? UInt,
            let reason = AVAudioSession.RouteChangeReason(rawValue: reasonValue),
            let routeDescription = userInfo[AVAudioSessionRouteChangePreviousRouteKey] as? AVAudioSessionRouteDescription else { return }
        switch reason {
        case .newDeviceAvailable:
            print("newDeviceAvailable")
        case .oldDeviceUnavailable:
            print("oldDeviceUnavailable")
        case .categoryChange:
            print("categoryChange")
            print("New category: \(AVAudioSession.sharedInstance().category)")
        case .override:
            print("override")
        case .wakeFromSleep:
            print("wakeFromSleep")
        case .noSuitableRouteForCategory:
            print("noSuitableRouteForCategory")
        case .routeConfigurationChange:
            print("routeConfigurationChange")
        case .unknown:
            print("unknown")
        @unknown default:
            fatalError("Really unknown reason: \(reason)")
        }
        
        print("Previous route:\n\(routeDescription)")
        print("Current route:\n\(AVAudioSession.sharedInstance().currentRoute)")
    }
    
    @objc
    func handleMediaServicesWereReset(_ notification: Notification) {
        resetUIStates()
        Task.init {
            resetAudioEngine()
            _ = await setupAudioEngine()
        }
    }
    
    func resetUIStates() {
//        fxSwitch.setOn(false, animated: true)
//        speechSwitch.setOn(false, animated: true)
//        bypassSwitch.setOn(false, animated: true)
//
//        recordButton.setTitle(ButtonTitles.record.rawValue, for: .normal)
//        recordButton.isEnabled = true
//        playButton.setTitle(ButtonTitles.play.rawValue, for: .normal)
//        playButton.isEnabled = false
    }
    
    func resetAudioEngine() {
        audioEngine = nil
    }
}


extension AVAudioUnit {
    static fileprivate func findComponent(type: String, subType: String, manufacturer: String) -> AVAudioUnitComponent? {
        // Make a component description matching any Audio Unit of the selected component type.
        let description = AudioComponentDescription(componentType: type.fourCharCode!,
                                                    componentSubType: subType.fourCharCode!,
                                                    componentManufacturer: manufacturer.fourCharCode!,
                                                    componentFlags: 0,
                                                    componentFlagsMask: 0)
        return AVAudioUnitComponentManager.shared().components(matching: description).first
    }
}

extension String {
    var fourCharCode: FourCharCode? {
        guard self.count == 4 && self.utf8.count == 4 else {
            return nil
        }

        var code: FourCharCode = 0
        for character in self.utf8 {
            code = code << 8 + FourCharCode(character)
        }
        return code
    }
}
