import Foundation
import SwiftUI
import UIKit
import CoreTypes
import Serde
import OSLog
import AVFoundation

@MainActor
class Core: ObservableObject {
    @Published var view: ViewModel

    @State var defaults: UserDefaults = UserDefaults()

    var startClock: ((
        @escaping(Double?) -> Void
    ) -> Void)?

    var stopClock: (() -> Void)?


    init() {
        self.view = try! .bincodeDeserialize(input: [UInt8](RedSiren.view()))
        logInit()
    }

    func update(_ event: Event) {
        let effects = [UInt8](processEvent(Data(try! event.bincodeSerialize())))
        let requests: [Request] = try! .bincodeDeserialize(input: effects)
        for request in requests {
            processEffect(request)
        }
    }

    func processEffect(_ request: Request) {
        switch request.effect {
        case .render:
            view = try! .bincodeDeserialize(input: [UInt8](RedSiren.view()))
        case .play(.runUnit):
            Logger().log("run unit")
            break
        case .play(.permissions):
            Task {
                let grant = await getRecordPermission()
                let data = try! [UInt8](UnitResolve.recordingPermission(grant).bincodeSerialize())
                let effects = [UInt8](handleResponse(Data(request.uuid), Data(data)))

                let requests: [Request] = try! .bincodeDeserialize(input: effects)
                for request in requests {
                    self.processEffect(request)
                }
            }
            break;
        case .animate(.start):
            Task {
                self.startClock!({ ts in
                    var data = try! AnimateOperationOutput.done.bincodeSerialize()
                    if let ts = ts {
                        data = try! AnimateOperationOutput.timestamp(ts).bincodeSerialize()
                        Logger().log("tick \(ts)")
                    }
                    else {
                        Logger().log("tick is none, animation is done")
                    }
                    let effects = [UInt8](handleResponse(Data(request.uuid), Data(data)))

                    let requests: [Request] = try! .bincodeDeserialize(input: effects)
                    for request in requests {
                        self.processEffect(request)
                    }
                })
            }
            break
        case .animate(.stop):
            self.stopClock!()
            break
        }
    }

    func getRecordPermission() async -> Bool {
        let status = AVCaptureDevice.authorizationStatus(for: .audio)
        var isAuthorized = status == .authorized
        if status == .notDetermined {
            isAuthorized = await AVCaptureDevice.requestAccess(for: .audio)
        }

        return isAuthorized
    }
}

protocol CoreEnv {
    func update(_ ev: Event) -> Void
}


struct CoreEnvProvider: CoreEnv {
    var core: Core
    init(core: Core) {
        self.core = core
    }

    @MainActor func update(_ ev: Event) {
        self.core.update(ev)
    }
}


struct CoreEnvKey: EnvironmentKey {
    static let defaultValue: CoreEnv? = nil
}

extension EnvironmentValues {
    var coreEnv: CoreEnv? {
        get { self[CoreEnvKey.self] }
        set { self[CoreEnvKey.self] = newValue }
    }
}
