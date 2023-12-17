import SharedTypes
import SwiftUI


struct InstrumentButtonView: View {
    var layoutRect: Rect
    var hSize: CGFloat
    var vSize: CGFloat
    var center: (CGFloat, CGFloat)

    init(rect: Rect) {
        self.layoutRect = rect
        let hSize = self.layoutRect.rect[1][0] - self.layoutRect.rect[0][0]
        let vSize = self.layoutRect.rect[1][1] - self.layoutRect.rect[0][1]
        let center = (self.layoutRect.rect[0][0] + hSize / 2, self.layoutRect.rect[0][1] + vSize / 2)

        self.hSize = CGFloat(hSize)
        self.vSize = CGFloat(vSize)
        self.center = (CGFloat(center.0), CGFloat(center.1))
    }

    var body: some View {

        Canvas { context, size in
            context.fill(
                Circle().path(in: CGRect(origin: .zero, size: size)),
                with: .color(Color("Primary"))
            )
        }
            .frame(width: self.hSize, height: self.vSize)
            .position(x: self.center.0, y: self.center.1)
    }

}

struct InstrumentStringView: View {
    var layoutLine: Line
    var hSize: CGFloat
    var vSize: CGFloat
    var center: (CGFloat, CGFloat)
    var p0: CGPoint
    var p1: CGPoint

    init(line: Line) {
        self.layoutLine = line
        let hSize = self.layoutLine.line[1][0] - self.layoutLine.line[0][0]
        let vSize = self.layoutLine.line[1][1] - self.layoutLine.line[0][1]
        let center = (self.layoutLine.line[0][0] + hSize / 2, self.layoutLine.line[0][1] + vSize / 2)

        self.hSize = CGFloat(hSize)
        self.vSize = CGFloat(vSize)
        self.center = (CGFloat(center.0), CGFloat(center.1))
        self.p0 = CGPoint(x: CGFloat(self.layoutLine.line[0][0]), y: CGFloat(self.layoutLine.line[0][1]))
        self.p1 = CGPoint(x: CGFloat(self.layoutLine.line[1][0]), y: CGFloat(self.layoutLine.line[1][1]))
    }

    var body: some View {

        Canvas { context, size in
            context.stroke(
                Path { path in

                    path.move(to: self.p0)

                    path.addLine(to: self.p1)

                },
                with: .color(Color("Primary")),
                lineWidth: 1
            )
        }
    }
}

struct InstrumentInboundStringView: View {
    var layoutLine: Line

    init(line: Line) {
        self.layoutLine = line
    }

    var body: some View {
        InstrumentStringView(line: self.layoutLine)
    }
}

struct InstrumentOutboundStringView: View {
    var layoutLine: Line

    init(line: Line) {
        self.layoutLine = line
    }

    var body: some View {
        InstrumentStringView(line: self.layoutLine)
    }
}

struct InstrumentTrackView: View {
    var layoutRect: Rect
    var hSize: CGFloat
    var vSize: CGFloat
    var center: (CGFloat, CGFloat)
    var r: CGSize

    init(rect: Rect) {
        self.layoutRect = rect
        let hSize = self.layoutRect.rect[1][0] - self.layoutRect.rect[0][0]
        let vSize = self.layoutRect.rect[1][1] - self.layoutRect.rect[0][1]
        let center = (self.layoutRect.rect[0][0] + hSize / 2, self.layoutRect.rect[0][1] + vSize / 2)
        let r = CGFloat(min(hSize, vSize)) / 2

        self.hSize = CGFloat(hSize)
        self.vSize = CGFloat(vSize)
        self.center = (CGFloat(center.0), CGFloat(center.1))
        self.r = CGSize(width: r, height: r)
    }

    var body: some View {

        Canvas { context, size in
            context.fill(
                RoundedRectangle(cornerSize: self.r).path(in: CGRect(origin: .zero, size: size)),
                with: .color(Color("Main"))
            )
            context.stroke(
                RoundedRectangle(cornerSize: self.r).path(in: CGRect(origin: CGPoint(x: 1, y: 1), size: CGSize(width: size.width - 2, height: size.height - 2))),
                with: .color(Color("Primary")),
                lineWidth: 1

            )
        }
            .frame(width: self.hSize, height: self.vSize)
            .position(x: self.center.0, y: self.center.1)

    }
}

struct InstrumentView: View {
    var vm: InstrumentVM
    var ev: (InstrumentEV) -> Void
    var vSize: CGFloat
    var hSize: CGFloat


    init(vm: InstrumentVM, ev: @escaping (InstrumentEV) -> Void) {
        self.vm = vm
        self.ev = ev
        self.vSize = vm.config.height
        self.hSize = vm.config.width
    }
    

    var body: some View {
        GeometryReader { proxy in
            ZStack {
                InstrumentInboundStringView(line: self.vm.layout.inbound)
                InstrumentOutboundStringView(line: self.vm.layout.outbound)
                
                ForEach(self.vm.layout.tracks, id: \.hashValue) { track in
                    InstrumentTrackView(rect: track)
                }
                
                ForEach(self.vm.layout.buttons, id: \.hashValue) { btn in
                    InstrumentButtonView(rect: btn)
                }
                
                MenuView(position: self.vm.layout.menu_position, expanded: false)
                
            }.ignoresSafeArea(.all)
                .frame(width: proxy.frame(in: .global).width, height: proxy.frame(in: .global).height)
        }.ignoresSafeArea(.all)


    }
}
