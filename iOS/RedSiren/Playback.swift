import Foundation
import AVFoundation
import AVFAudio
import CoreTypes
import UIKit
import SwiftUI
import OSLog





class Playback: NSObject, ObservableObject {


    private var session: AVAudioSession?
    private var auCore: AuCoreBridge?

    override init() {

    }

    public func request(_ op: PlayOperation, onData: @escaping (_ data: Data) -> Void) -> Void {
        switch op {
        case .permissions:
            Task {
                let grant = await isAuthorized
                let data = try! [UInt8](PlayOperationOutput.permission(grant).bincodeSerialize())
                onData(Data(data))
                
                Logger().log("playback permissions task complete")
            }
        case .installAU:
            guard setupAudioSession() else {
                let data = try! PlayOperationOutput.success(false).bincodeSerialize()

                onData(Data(data))
                
                return
            }
            auCore = auNew()
            do {
                let opData = try op.bincodeSerialize()
                let rcv = auRequest(self.auCore!, Data.init(opData))

                Task {
                    if let data = await auReceive(rcv) {
                        onData(data)
                    }
                    
                    Logger().log("playback install au task complete")
                }
            }
            catch {
                let data = try! [UInt8](PlayOperationOutput.success(false).bincodeSerialize())
                onData(Data(data))
            }
        default:
            do {
                let opData = try op.bincodeSerialize()
                let rcv = auRequest(self.auCore!, Data.init(opData))

                Task {
                    while let data = await auReceive(rcv) {
                        onData(data)
                    }
                    
                    Logger().log("playback forwarded task complete")
                }
            }
            catch {
                let data = try! [UInt8](PlayOperationOutput.success(false).bincodeSerialize())
                onData(Data(data))
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
