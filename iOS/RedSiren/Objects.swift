import SwiftUI
import CoreTypes

struct Objects: View {
    @EnvironmentObject var core: Core
    
    func object(_ obj: ViewObject, _ paint: Paint) -> AnyView {
        switch obj.shape {
            case .circle(let box)
            Circle().frame(width: box.max[0] - box.min[0], height: box.max[1] - box.min[1]).position(x: box.min[])
        }
    }
    
    var body: some View {
        ForEach(core.view.visual.objects, id: \.field0.hashValue) { obj in
            object(obj.field0, obj.field1)
        }
    }
}
