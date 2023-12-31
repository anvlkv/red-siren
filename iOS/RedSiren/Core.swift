import Foundation
import SwiftUI
import UIKit
import SharedTypes
import Serde
import OSLog

@MainActor
class Core: ObservableObject {
    @Published var view: ViewModel

    @State var playback: Playback = Playback()
    
    var startClock: ((
        @escaping(Double) -> Void
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
        case let .navigate(.to(activity)):
            self.update(Event.reflectActivity(activity))
            break
        case .keyValue(.read):
            let response = KeyValueOutput.read(.none)

            let effects = [UInt8](handleResponse(Data(request.uuid), Data(try! response.bincodeSerialize())))

            let requests: [Request] = try! .bincodeDeserialize(input: effects)
            for request in requests {
                processEffect(request)
            }
        case .keyValue(.write):
            let response = KeyValueOutput.write(false)

            let effects = [UInt8](handleResponse(Data(request.uuid), Data(try! response.bincodeSerialize())))

            let requests: [Request] = try! .bincodeDeserialize(input: effects)
            for request in requests {
                processEffect(request)
            }
        case .play(let op):
            Task {
                let response = await playback.request(op)


                let effects = [UInt8](handleResponse(Data(request.uuid), Data(response)))

                let requests: [Request] = try! .bincodeDeserialize(input: effects)
                for request in requests {
                    processEffect(request)
                }
            }
            break
        case .animate(.start):
            Task {
                self.startClock!({ ts in
                    let data = try! AnimateOperationOutput.timestamp(ts).bincodeSerialize()
                    let effects = [UInt8](handleResponse(Data(request.uuid), Data(data)))

                    let requests: [Request] = try! .bincodeDeserialize(input: effects)
                    for request in requests {
                        self.processEffect(request)
                    }
                })
            }
            break
        case .animate(.stop):
            Task {
                self.stopClock!()
                let data = try! AnimateOperationOutput.done.bincodeSerialize()
                let effects = [UInt8](handleResponse(Data(request.uuid), Data(data)))

                let requests: [Request] = try! .bincodeDeserialize(input: effects)
                for request in requests {
                    self.processEffect(request)
                }
            }
            break
        }

        
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
