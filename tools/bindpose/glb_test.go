package main

import (
	"encoding/binary"
	"testing"
)

func TestReadGLBTruncated(t *testing.T) {
	// A header and a JSON chunk claiming more bytes than the file has.
	b := make([]byte, 24)
	copy(b, "glTF")
	binary.LittleEndian.PutUint32(b[4:], 2)
	binary.LittleEndian.PutUint32(b[8:], 64)
	binary.LittleEndian.PutUint32(b[12:], 40)
	copy(b[16:], "JSON")
	if _, _, err := readGLB(b); err == nil {
		t.Fatal("read a truncated .glb without an error")
	}
}

func TestInverseBindsDefault(t *testing.T) {
	g := &gltf{}
	got, err := g.inverseBinds(obj{"joints": []any{0.0, 1.0, 2.0}})
	if err != nil {
		t.Fatal(err)
	}
	if len(got) != 3 {
		t.Fatalf("got %d matrices, want 3", len(got))
	}
	for i, m := range got {
		if m != identity() {
			t.Errorf("matrix %d is %v, want identity", i, m)
		}
	}
}
