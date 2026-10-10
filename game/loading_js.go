package game

import "syscall/js"

var loadingComplete bool

func notifyFirstFrame() {
	if loadingComplete {
		return
	}
	loadingComplete = true
	if ready := js.Global().Get("earthTwoReady"); ready.Type() == js.TypeFunction {
		ready.Invoke()
	}
}
