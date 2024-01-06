import CoreTypes
import SwiftUI

struct TunerFFTView: View {
    var layoutLine: Line
    var hSize: CGFloat
    var vSize: CGFloat
    var center: (CGFloat, CGFloat)
    var p0: CGPoint
    var p1: CGPoint
    var data: [[Float]]

    init(line: Line, data: [[Float]]) {
        self.layoutLine = line
        let hSize = self.layoutLine.line[1][0] - self.layoutLine.line[0][0]
        let vSize = self.layoutLine.line[1][1] - self.layoutLine.line[0][1]
        let center = (self.layoutLine.line[0][0] + hSize / 2, self.layoutLine.line[0][1] + vSize / 2)

        self.hSize = CGFloat(hSize)
        self.vSize = CGFloat(vSize)
        self.center = (CGFloat(center.0), CGFloat(center.1))
        self.p0 = CGPoint(x: CGFloat(self.layoutLine.line[0][0]), y: CGFloat(self.layoutLine.line[0][1]))
        self.p1 = CGPoint(x: CGFloat(self.layoutLine.line[1][0]), y: CGFloat(self.layoutLine.line[1][1]))
        self.data = data
    }

    var body: some View {

        Canvas { context, size in
            context.stroke(
                Path { path in

                    path.move(to: self.p0)
                    
                    let step = self.hSize  / CGFloat(self.data.count)
                    
                    for (index, val) in Array(self.data.enumerated()) {
                        let x = self.p0.x + step * CGFloat(index)
                        let y = self.p0.y + CGFloat(val[1])
                        path.addLine(to: CGPoint(x: x, y: y))
                    }

                    path.addLine(to: self.p1)

                },
                with: .color(Color("Primary")),
                lineWidth: 1
            )
        }
    }
}

struct TunerView: View {
    var vm: TunerVM
    var ev: (TunerEV) -> Void
    var vSize: CGFloat
    var hSize: CGFloat


    init(vm: TunerVM, ev: @escaping (TunerEV) -> Void, vSize: CGFloat, hSize: CGFloat) {
        self.vm = vm
        self.ev = ev
        self.vSize = vSize
        self.hSize = hSize
    }
    

    var body: some View {
        GeometryReader { proxy in
            ZStack {
                TunerFFTView(line: vm.line, data: vm.fft)
                
                ForEach(self.vm.pairs, id: \.f_n) { btn in
                    InstrumentButtonView(rect: btn.rect)
                }
                
            }.ignoresSafeArea(.all)
                .frame(width: proxy.frame(in: .global).width, height: proxy.frame(in: .global).height)
        }.ignoresSafeArea(.all)


    }
}
