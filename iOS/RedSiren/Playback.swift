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
    private var auCore: AuCoreBridge?

    override init() {

    }

    public func request(_ op: PlayOperation) async -> [UInt8] {
        switch op {
        case .permissions:
            let grant = await isAuthorized
            return try! [UInt8](PlayOperationOutput.permission(grant).bincodeSerialize())
        case .installAU:
            guard setupAudioSession() else {
                let opData = try! PlayOperationOutput.success(false).bincodeSerialize()
                let data = await auRequest(self.auCore!, Data.init(opData))

                return [UInt8](data)
            }
            auCore = auNew()
            do {
                let opData = try op.bincodeSerialize()
                let data = await auRequest(self.auCore!, Data.init(opData))

                return [UInt8](data)
            }
            catch {
                return try! [UInt8](PlayOperationOutput.success(false).bincodeSerialize())
            }
        default:
            do {
                let opData = try op.bincodeSerialize()
                let data = await auRequest(self.auCore!, Data.init(opData))

                return [UInt8](data)
            }
            catch {
                return try! [UInt8](PlayOperationOutput.success(false).bincodeSerialize())
            }

        }
    }



    var isAuthorized: Bool {
        get async {
            let status = AVCaptureDevice.authorizationStatus(for: .audio)
            var isAuthorized = status == .authorized
            if status == .notDetermined {
                isAuthorized = await AVCaptureDevice.requestAccess(for: .audio)
            }

            return isAuthorized
        }
    }

    func setupAudioSession() -> Bool {
        self.session = AVAudioSession.sharedInstance()

        do {
            try session!.setCategory(.playAndRecord, mode: .measurement, options: .defaultToSpeaker)
        } catch {
            print("Could not set the audio category: \(error.localizedDescription)")
            return false
        }
        
        

         do {
             try session!.setPreferredSampleRate(44100)
         } catch {
             print("Could not set the preferred sample rate: \(error.localizedDescription)")
             return false
         }

        do {
            try session!.setActive(true)
        }
        catch {
            return false
        }

        return true
    }
}
