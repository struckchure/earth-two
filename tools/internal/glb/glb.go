// Package glb reads and writes binary glTF files.
package glb

import (
	"bytes"
	"encoding/binary"
	"encoding/json"
	"errors"
)

// Read splits a .glb file into its JSON document and its binary chunk.
func Read(b []byte) (map[string]any, []byte, error) {
	if len(b) < 20 || string(b[:4]) != "glTF" {
		return nil, nil, errors.New("not a .glb file")
	}
	var doc map[string]any
	var bin []byte
	for off := 12; off+8 <= len(b); {
		n := int(binary.LittleEndian.Uint32(b[off:]))
		if n < 0 || n > len(b)-off-8 {
			return nil, nil, errors.New("truncated .glb file")
		}
		kind := string(b[off+4 : off+8])
		chunk := b[off+8 : off+8+n]
		switch kind {
		case "JSON":
			if err := json.Unmarshal(chunk, &doc); err != nil {
				return nil, nil, err
			}
		case "BIN\x00":
			bin = append([]byte(nil), chunk...)
		}
		off += 8 + n
	}
	if doc == nil {
		return nil, nil, errors.New("no JSON chunk")
	}
	return doc, bin, nil
}

// Write is the .glb file of doc and bin, padding bin and setting the
// document's buffer to it.
func Write(doc map[string]any, bin []byte) ([]byte, error) {
	for len(bin)%4 != 0 {
		bin = append(bin, 0)
	}
	doc["buffers"] = []any{map[string]any{"byteLength": len(bin)}}
	js, err := json.Marshal(doc)
	if err != nil {
		return nil, err
	}
	for len(js)%4 != 0 {
		js = append(js, ' ')
	}
	out := new(bytes.Buffer)
	le := binary.LittleEndian
	binary.Write(out, le, [3]uint32{0x46546C67, 2, uint32(12 + 8 + len(js) + 8 + len(bin))})
	binary.Write(out, le, [2]uint32{uint32(len(js)), 0x4E4F534A})
	out.Write(js)
	binary.Write(out, le, [2]uint32{uint32(len(bin)), 0x004E4942})
	out.Write(bin)
	return out.Bytes(), nil
}
