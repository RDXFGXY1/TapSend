package updates

import (
	"encoding/json"
	"fmt"
	"strings"
	"testing"
)

func fixtureRelease(tag string, draft, prerelease, bothAssets bool) string {
	prefix := "TapSend-" + tag + "-windows-x64"
	assets := fmt.Sprintf(`{"name":%q,"state":"uploaded"}`, prefix+".zip")
	if bothAssets {
		assets += fmt.Sprintf(`,{"name":%q,"state":"uploaded"}`, prefix+"-setup.exe")
	}
	return fmt.Sprintf(`{"tag_name":%q,"draft":%t,"prerelease":%t,"assets":[%s]}`,
		tag, draft, prerelease, assets)
}

func TestSelectWindowsRelease(t *testing.T) {
	input := "[" + strings.Join([]string{
		fixtureRelease("v3.0.0", true, false, true),
		fixtureRelease("v2.0.0-alpha.1", false, true, true),
		fixtureRelease("v1.20.0", false, false, false),
		fixtureRelease("v1.2.9", false, false, true),
		fixtureRelease("v1.10.0", false, false, true),
		fixtureRelease("v1.2.10", false, false, true),
	}, ",") + "]"
	if !json.Valid([]byte(input)) {
		t.Fatal("bad release fixture")
	}
	const repo = "RDXFGXY1/TapSend"
	update, err := selectWindowsRelease(strings.NewReader(input), repo, "1.2.9")
	if err != nil || update == nil || update.Tag != "v1.10.0" ||
		update.URL != "https://github.com/"+repo+"/releases/tag/v1.10.0" {
		t.Fatalf("update = %#v, error = %v", update, err)
	}
	update, err = selectWindowsRelease(strings.NewReader(input), repo, "1.10.0")
	if err != nil || update != nil {
		t.Fatalf("current release offered an update: %#v, %v", update, err)
	}
	update, err = selectWindowsRelease(strings.NewReader(input), repo, "1.10.0-rc.1")
	if err != nil || update == nil || update.Tag != "v1.10.0" {
		t.Fatalf("prerelease did not offer the matching stable update: %#v, %v", update, err)
	}
}
