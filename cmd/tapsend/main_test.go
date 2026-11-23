package main

import (
	"reflect"
	"testing"
)

func TestSendOptionsCanBeReorderedAndMixedWithPositionals(t *testing.T) {
	cases := []struct {
		args   []string
		device string
		paths  []string
	}{
		{[]string{"-d", "My Phone", "-p", "one file.txt"}, "My Phone", []string{"one file.txt"}},
		{[]string{"--path", "one.txt", "--device", "My Phone", "-p", "two.txt"}, "My Phone", []string{"one.txt", "two.txt"}},
		{[]string{"My Phone", "one.txt"}, "My Phone", []string{"one.txt"}},
		{[]string{"-p", "one.txt", "My Phone"}, "My Phone", []string{"one.txt"}},
		{[]string{"-d", "My Phone", "--", "-starts-with-dash.txt"}, "My Phone", []string{"-starts-with-dash.txt"}},
	}
	for _, test := range cases {
		got, err := parseSend(test.args)
		if err != nil {
			t.Fatalf("%v: %v", test.args, err)
		}
		if got.device != test.device || !reflect.DeepEqual(got.paths, test.paths) {
			t.Fatalf("%v: got %+v", test.args, got)
		}
	}
}

func TestSendRejectsAmbiguousExtraPositionals(t *testing.T) {
	if _, err := parseSend([]string{"-d", "phone", "-p", "file.txt", "extra.txt"}); err == nil {
		t.Fatal("extra positional path should require -p")
	}
}
