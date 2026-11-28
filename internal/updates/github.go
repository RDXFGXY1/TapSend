package updates

import (
	"context"
	"encoding/json"
	"errors"
	"fmt"
	"io"
	"net/http"
	"regexp"
	"strconv"
	"strings"
	"time"
)

var repositoryName = regexp.MustCompile(`^[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+$`)

type Release struct {
	Tag string
	URL string
}

func Check(ctx context.Context, repository, current string) (*Release, error) {
	if !repositoryName.MatchString(repository) {
		return nil, errors.New("invalid GitHub repository")
	}
	client := &http.Client{Timeout: 8 * time.Second}
	endpoint := "https://api.github.com/repos/" + repository + "/releases?per_page=20"
	request, err := http.NewRequestWithContext(ctx, http.MethodGet, endpoint, nil)
	if err != nil {
		return nil, err
	}
	request.Header.Set("Accept", "application/vnd.github+json")
	request.Header.Set("User-Agent", "TapSend-Windows/"+current)
	response, err := client.Do(request)
	if err != nil {
		return nil, err
	}
	defer response.Body.Close()
	if response.StatusCode != http.StatusOK {
		return nil, fmt.Errorf("GitHub returned HTTP %d", response.StatusCode)
	}
	return selectWindowsRelease(io.LimitReader(response.Body, 2<<20), repository, current)
}

type releaseInfo struct {
	Tag        string `json:"tag_name"`
	Draft      bool
	Prerelease bool
	Assets     []struct {
		Name, State string
	}
}

func selectWindowsRelease(input io.Reader, repository, current string) (*Release, error) {
	baseline, ok := parseVersion(strings.SplitN(current, "-", 2)[0])
	if !ok {
		return nil, errors.New("invalid installed version")
	}
	var releases []releaseInfo
	if err := json.NewDecoder(input).Decode(&releases); err != nil {
		return nil, err
	}
	var result *Release
	for _, release := range releases {
		version, ok := parseVersion(strings.TrimPrefix(release.Tag, "v"))
		if !ok || !strings.HasPrefix(release.Tag, "v") || release.Draft || release.Prerelease {
			continue
		}
		if order := compareVersion(version, baseline); order < 0 || (order == 0 && (!strings.Contains(current, "-") || result != nil)) {
			continue
		}
		prefix := "TapSend-" + release.Tag + "-windows-x64"
		var installer, bundle bool
		for _, asset := range release.Assets {
			if asset.State == "uploaded" {
				installer = installer || asset.Name == prefix+"-setup.exe"
				bundle = bundle || asset.Name == prefix+".zip"
			}
		}
		if installer && bundle {
			baseline = version
			result = &Release{release.Tag, "https://github.com/" + repository + "/releases/tag/" + release.Tag}
		}
	}
	return result, nil
}

func parseVersion(value string) ([3]uint64, bool) {
	var result [3]uint64
	parts := strings.Split(value, ".")
	if len(parts) != len(result) {
		return result, false
	}
	for i, part := range parts {
		n, err := strconv.ParseUint(part, 10, 32)
		if err != nil || strconv.FormatUint(n, 10) != part {
			return result, false
		}
		result[i] = n
	}
	return result, true
}

func compareVersion(a, b [3]uint64) int {
	for i, part := range a {
		if part < b[i] {
			return -1
		}
		if part > b[i] {
			return 1
		}
	}
	return 0
}
