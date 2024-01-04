import Foundation
import SwiftUI
import UIKit
import CoreTypes
import Serde
import OSLog

@MainActor
class Core: ObservableObject {
    @Published var view: ViewModel

    @State var playback: Playback = Playback()
    
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
        case let .navigate(.to(activity)):
            self.update(Event.reflectActivity(activity))
            break
        case .keyValue(.read(let key)):
            
            var response = KeyValueOutput.read(.none)
            
            if let data = self.defaults.array(forKey: key) {
                response = KeyValueOutput.read(.some(data as! [UInt8]))
                Logger().log("restore data for \(key)")
            }
            else {
                Logger().log("no data for \(key)")
            }
            

            let effects = [UInt8](handleResponse(Data(request.uuid), Data(try! response.bincodeSerialize())))

            let requests: [Request] = try! .bincodeDeserialize(input: effects)
            for request in requests {
                processEffect(request)
            }
        case .keyValue(.write(let key, let data)):
            self.defaults.setValue(data, forKey: key)
            
            let response = KeyValueOutput.write(true)

            let effects = [UInt8](handleResponse(Data(request.uuid), Data(try! response.bincodeSerialize())))

            let requests: [Request] = try! .bincodeDeserialize(input: effects)
            for request in requests {
                processEffect(request)
            }
        case .play(let op):
            playback.request(op) { response in
                DispatchQueue.main.async {
                    let effects = [UInt8](handleResponse(Data(request.uuid), Data(response)))

                    let requests: [Request] = try! .bincodeDeserialize(input: effects)
                    for request in requests {
                        self.processEffect(request)
                    }
                }
            }
            break
        case .animate(.start):
            self.startClock!({ ts in
                var data = try! AnimateOperationOutput.done.bincodeSerialize()
                if let ts = ts {
                    data = try! AnimateOperationOutput.timestamp(ts).bincodeSerialize()
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
            break
        case .animate(.stop):
            self.stopClock!()
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
