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
    
    private var session: AVAudioSession?
    private var audioEngine: AVAudioEngine?
    
    let config: Config
    let ev: (InstrumentEV) -> Void
    
    init(config: Config, ev: @escaping (InstrumentEV) -> Void) {
        self.config=config
        self.ev = ev
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
        
        let input = audioEngine!.inputNode
        
//        audioEngine?.attach(<#T##node: AVAudioNode##AVAudioNode#>)
        
        return true
    }
    
    public func evPort(event: Event) {
        
    }
    
    public func setupAudioSession() {
        let session = AVAudioSession.sharedInstance()

        do {
            try session.setCategory(.playAndRecord, mode: .measurement, options: .defaultToSpeaker)
        } catch {
            print("Could not set the audio category: \(error.localizedDescription)")
        }

        do {
            try session.setPreferredSampleRate(self.config.sample_rate_hz)
        } catch {
            print("Could not set the preferred sample rate: \(error.localizedDescription)")
        }
        
        let task = Task.init{
            let ready = await setUpCaptureSession()
            
            if ready {
                print("audio session ready")
            }
            else {
                print("audio session was not allowed to start")
            }
        }
    }
    
    func setupAudioEngine() {
        do {
            audioEngine = AVAudioEngine()
            setupAudioSession()
            try audioEngine!.start()
        } catch {
            fatalError("Could not set up the audio engine: \(error)")
        }
    }
    
    @objc
    func handleInterruption(_ notification: Notification) {
        guard let userInfo = notification.userInfo,
            let typeValue = userInfo[AVAudioSessionInterruptionTypeKey] as? UInt,
            let type = AVAudioSession.InterruptionType(rawValue: typeValue) else { return }
        
        switch type {
        case .began:
            break
        case .ended:
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
        resetAudioEngine()
        setupAudioEngine()
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


