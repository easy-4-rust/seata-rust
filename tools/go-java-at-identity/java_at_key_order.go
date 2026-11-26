// Copyright 2026 Seata-Rust contributors. Licensed under the Apache License, Version 2.0.
package sql

import (
	"errors"
	"seata.apache.org/seata-go/v2/pkg/datasource/sql/types"
	"unicode/utf16"
)

// javaATKeyNode只复现固定JDK17 HashMap<String,...>插入后的遍历次序；MySQL主键最多16列。
type javaATKeyNode struct {
	key                             string
	hash                            uint32
	next, prev, parent, left, right *javaATKeyNode
	red                             bool
}

func javaATHash(key string) uint32 {
	var h uint32
	for _, unit := range utf16.Encode([]rune(key)) {
		h = 31*h + uint32(unit)
	}
	return h ^ (h >> 16)
}
func javaATCompare(a, b string) int {
	aa, bb := utf16.Encode([]rune(a)), utf16.Encode([]rune(b))
	for i := 0; i < len(aa) && i < len(bb); i++ {
		if aa[i] < bb[i] {
			return -1
		}
		if aa[i] > bb[i] {
			return 1
		}
	}
	return len(aa) - len(bb)
}
func javaATNodeLess(a, b *javaATKeyNode) bool {
	if a.hash != b.hash {
		return int32(a.hash) < int32(b.hash)
	}
	return javaATCompare(a.key, b.key) < 0
}
func javaATRotateLeft(root, x *javaATKeyNode) *javaATKeyNode {
	y := x.right
	x.right = y.left
	if y.left != nil {
		y.left.parent = x
	}
	y.parent = x.parent
	if x.parent == nil {
		root = y
	} else if x == x.parent.left {
		x.parent.left = y
	} else {
		x.parent.right = y
	}
	y.left = x
	x.parent = y
	return root
}
func javaATRotateRight(root, x *javaATKeyNode) *javaATKeyNode {
	y := x.left
	x.left = y.right
	if y.right != nil {
		y.right.parent = x
	}
	y.parent = x.parent
	if x.parent == nil {
		root = y
	} else if x == x.parent.right {
		x.parent.right = y
	} else {
		x.parent.left = y
	}
	y.right = x
	x.parent = y
	return root
}
func javaATBalance(root, x *javaATKeyNode) *javaATKeyNode {
	x.red = true
	for x != root && x.parent != nil && x.parent.red {
		p := x.parent
		g := p.parent
		if g == nil {
			break
		}
		if p == g.left {
			u := g.right
			if u != nil && u.red {
				p.red = false
				u.red = false
				g.red = true
				x = g
				continue
			}
			if x == p.right {
				x = p
				root = javaATRotateLeft(root, x)
				p = x.parent
				g = p.parent
			}
			p.red = false
			g.red = true
			root = javaATRotateRight(root, g)
		} else {
			u := g.left
			if u != nil && u.red {
				p.red = false
				u.red = false
				g.red = true
				x = g
				continue
			}
			if x == p.left {
				x = p
				root = javaATRotateRight(root, x)
				p = x.parent
				g = p.parent
			}
			p.red = false
			g.red = true
			root = javaATRotateLeft(root, g)
		}
	}
	root.red = false
	return root
}
func javaATMoveRoot(head, root *javaATKeyNode) *javaATKeyNode {
	if head == root {
		return head
	}
	if root.prev != nil {
		root.prev.next = root.next
	}
	if root.next != nil {
		root.next.prev = root.prev
	}
	head.prev = root
	root.next = head
	root.prev = nil
	return root
}
func javaATTreeify(head *javaATKeyNode) *javaATKeyNode {
	var root *javaATKeyNode
	for x := head; x != nil; x = x.next {
		x.parent = nil
		x.left = nil
		x.right = nil
		x.red = false
		if root == nil {
			root = x
			continue
		}
		p := root
		for {
			if javaATNodeLess(x, p) {
				if p.left == nil {
					p.left = x
					x.parent = p
					break
				}
				p = p.left
			} else {
				if p.right == nil {
					p.right = x
					x.parent = p
					break
				}
				p = p.right
			}
		}
		root = javaATBalance(root, x)
	}
	return javaATMoveRoot(head, root)
}
func javaATTreePut(head, x *javaATKeyNode) *javaATKeyNode {
	root := head
	for root.parent != nil {
		root = root.parent
	}
	p := root
	for {
		if javaATNodeLess(x, p) {
			if p.left == nil {
				p.left = x
				break
			}
			p = p.left
		} else {
			if p.right == nil {
				p.right = x
				break
			}
			p = p.right
		}
	}
	x.parent = p
	x.prev = p
	x.next = p.next
	if p.next != nil {
		p.next.prev = x
	}
	p.next = x
	return javaATMoveRoot(head, javaATBalance(root, x))
}
func javaATPrimaryKeyOrder(meta *types.TableMeta) ([]string, error) {
	invalid := errors.New("Java AT: invalid MySQL primary-key definition")
	var input []string
	indexes := 0
	for _, index := range meta.Indexs {
		if index.IType == types.IndexTypePrimaryKey {
			indexes++
			for _, column := range index.Columns {
				input = append(input, column.ColumnName)
			}
		}
	}
	if indexes != 1 || len(input) == 0 || len(input) > 16 {
		return nil, invalid
	}
	buckets := make([]*javaATKeyNode, 16)
	trees := make([]bool, 16)
	size := 0
	seen := map[string]bool{}
	resize := func() {
		next := make([]*javaATKeyNode, len(buckets)*2)
		tails := make([]*javaATKeyNode, len(next))
		for _, head := range buckets {
			for head != nil {
				x := head
				head = head.next
				x.next = nil
				x.prev = nil
				slot := int(x.hash & uint32(len(next)-1))
				if tails[slot] == nil {
					next[slot] = x
				} else {
					tails[slot].next = x
					x.prev = tails[slot]
				}
				tails[slot] = x
			}
		}
		buckets = next
		trees = make([]bool, len(next))
	}
	for _, key := range input {
		if key == "" || seen[key] {
			return nil, invalid
		}
		seen[key] = true
		x := &javaATKeyNode{key: key, hash: javaATHash(key)}
		slot := int(x.hash & uint32(len(buckets)-1))
		if buckets[slot] == nil {
			buckets[slot] = x
		} else if trees[slot] {
			buckets[slot] = javaATTreePut(buckets[slot], x)
		} else {
			tail := buckets[slot]
			count := 1
			for tail.next != nil {
				tail = tail.next
				count++
			}
			tail.next = x
			x.prev = tail
			if count >= 8 {
				if len(buckets) < 64 {
					resize()
				} else {
					buckets[slot] = javaATTreeify(buckets[slot])
					trees[slot] = true
				}
			}
		}
		size++
		if size > len(buckets)*3/4 {
			resize()
		}
	}
	order := make([]string, 0, len(input))
	for _, head := range buckets {
		for x := head; x != nil; x = x.next {
			order = append(order, x.key)
		}
	}
	return order, nil
}
