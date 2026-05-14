package tree_sitter_surface_test

import (
	"testing"

	tree_sitter "github.com/smacker/go-tree-sitter"
	"github.com/tree-sitter/tree-sitter-surface"
)

func TestCanLoadGrammar(t *testing.T) {
	language := tree_sitter.NewLanguage(tree_sitter_surface.Language())
	if language == nil {
		t.Errorf("Error loading Surface grammar")
	}
}
