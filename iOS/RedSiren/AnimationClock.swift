import Foundation
import UIKit

@MainActor
class AnimationClock: NSObject, ObservableObject {
    @Published var ts: Double = 0
    var onTick: (Double) -> Void
    var onStart: (Double) -> Void
    
    var link: CADisplayLink?
    
    init(onTick: @escaping (Double) -> Void = {_ in}, onStart: @escaping (Double) -> Void = {_ in}) {
        self.onTick = onTick
        self.onStart = onStart
    }
    
    func createDisplayLink() {
        self.deleteDisplayLink()
        
        self.link = CADisplayLink(target: self,
                                        selector: #selector(step))
        
        self.link!.add(to: .current,
                        forMode: RunLoop.Mode.default)
    }
    
    func deleteDisplayLink() {
        self.link?.invalidate()
        self.link = nil
    }
    
    @objc func step(displaylink: CADisplayLink) {
        let old_ts = self.ts
        self.ts = displaylink.targetTimestamp * 1000
    
        if old_ts == 0 {
            self.onStart(self.ts)
        }
        else {
            self.onTick(self.ts)
        }
    }
}
