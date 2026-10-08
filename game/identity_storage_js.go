//go:build js && wasm

package game

import (
	"context"
	"encoding/hex"
	"encoding/json"
	"errors"
	"syscall/js"
)

func loadIdentityBackup() (data []byte, err error) {
	defer func() {
		if recover() != nil {
			err = errors.New("browser storage is unavailable")
		}
	}()
	v := js.Global().Get("localStorage").Call("getItem", "earth-two/identity/active")
	if !v.IsNull() {
		data = []byte(v.String())
	}
	return
}
func saveIdentityBackup(data []byte) (err error) {
	defer func() {
		if recover() != nil {
			err = errors.New("could not save encrypted identity in browser storage")
		}
	}()
	storage := js.Global().Get("localStorage")
	old := storage.Call("getItem", "earth-two/identity/active")
	if !old.IsNull() {
		if id := identityBackupID([]byte(old.String())); id != "" {
			storage.Call("setItem", "earth-two/identity/saved/"+id, old.String())
		}
	}
	storage.Call("setItem", "earth-two/identity/active", string(data))
	var metadata struct {
		PublicKey []byte `json:"public_key"`
	}
	if err := json.Unmarshal(data, &metadata); err != nil {
		return err
	}
	storage.Call("setItem", "earth-two/identity/pub", hex.EncodeToString(metadata.PublicKey))
	return nil
}
func defaultIdentityTransfer() string {
	return "Import opens a file chooser; export downloads your key"
}
func readIdentityTransfer(ctx context.Context, _ string) ([]byte, error) {
	document := js.Global().Get("document")
	input := document.Call("createElement", "input")
	input.Set("type", "file")
	input.Set("accept", ".json,application/json")
	input.Get("style").Set("display", "none")
	document.Get("body").Call("appendChild", input)
	files := make(chan js.Value, 1)
	change := js.FuncOf(func(_ js.Value, _ []js.Value) any {
		list := input.Get("files")
		if list.Get("length").Int() > 0 {
			files <- list.Index(0)
		} else {
			files <- js.Null()
		}
		return nil
	})
	cancel := js.FuncOf(func(_ js.Value, _ []js.Value) any { files <- js.Null(); return nil })
	input.Set("onchange", change)
	input.Set("oncancel", cancel)
	defer func() {
		input.Set("onchange", js.Null())
		input.Set("oncancel", js.Null())
		input.Call("remove")
		change.Release()
		cancel.Release()
	}()
	input.Call("click")
	var file js.Value
	select {
	case <-ctx.Done():
		return nil, ctx.Err()
	case file = <-files:
	}
	if file.IsNull() {
		return nil, errors.New("key import cancelled")
	}
	if file.Get("size").Int() > 4096 {
		return nil, errors.New("key backup is too large")
	}
	out := make(chan string, 1)
	var success, failure js.Func
	success = js.FuncOf(func(_ js.Value, args []js.Value) any {
		out <- args[0].String()
		success.Release()
		failure.Release()
		return nil
	})
	failure = js.FuncOf(func(_ js.Value, _ []js.Value) any { out <- ""; success.Release(); failure.Release(); return nil })
	file.Call("text").Call("then", success, failure)
	select {
	case <-ctx.Done():
		return nil, ctx.Err()
	case text := <-out:
		if text == "" {
			return nil, errors.New("could not read key backup")
		}
		return []byte(text), nil
	}
}
func exportIdentityTransfer(data []byte, _ string) error {
	bytes := js.Global().Get("Uint8Array").New(len(data))
	js.CopyBytesToJS(bytes, data)
	blob := js.Global().Get("Blob").New([]any{bytes}, map[string]any{"type": "application/json"})
	url := js.Global().Get("URL").Call("createObjectURL", blob)
	anchor := js.Global().Get("document").Call("createElement", "a")
	anchor.Set("href", url)
	anchor.Set("download", identityBackupID(data)+".earth-two-key.json")
	anchor.Call("click")
	var revoke js.Func
	revoke = js.FuncOf(func(_ js.Value, _ []js.Value) any {
		js.Global().Get("URL").Call("revokeObjectURL", url)
		revoke.Release()
		return nil
	})
	js.Global().Call("setTimeout", revoke, 1000)
	return nil
}

func identityPasteText(_ bool) string {
	b := js.Global().Get("EarthTwoAccount")
	if b.IsUndefined() || b.IsNull() {
		return ""
	}
	return b.Call("takePaste").String()
}
func identityClipboardFocus(active bool) {
	b := js.Global().Get("EarthTwoAccount")
	if !b.IsUndefined() && !b.IsNull() {
		b.Call("capturePaste", active)
	}
}
func identityNetworkDefaults() (string, string) {
	params := js.Global().Get("URLSearchParams").New(js.Global().Get("location").Get("search"))
	host, db := "http://localhost:3000", "earth-two"
	if v := params.Call("get", "server"); !v.IsNull() {
		host = v.String()
	}
	if v := params.Call("get", "database"); !v.IsNull() {
		db = v.String()
	}
	return host, db
}
