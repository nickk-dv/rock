#ifndef TREE_H
#define TREE_H

#include "arena.h"

template<typename T>
struct Tree_Node
{
	T value;
	Tree_Node* parent;
	Tree_Node* next_child;
};

template<typename T>
struct Tree
{
	Arena arena;
	Tree_Node<T>* root;

	Tree(u64 block_size, T root_value)
	{
		arena_init(&arena, block_size);
		root = arena_alloc<Tree_Node<T>>(&arena);
		root->value = root_value;
		root->parent = nullptr;
		root->next_child = nullptr;
	}

	~Tree() { arena_deinit(&arena); }
};

template<typename T>
Tree_Node<T>* tree_node_add_child(Arena* arena, Tree_Node<T>* parent, T value)
{
	Tree_Node<T>* node = arena_alloc<Tree_Node<T>>(arena);
	node->value = value;
	node->parent = parent;
	node->next_child = nullptr;
	
	if (parent->next_child == nullptr) 
	{
		parent->next_child = node;
		return node;
	}

	Tree_Node<T>* curr = parent->next_child;
	while (curr->next_child != nullptr) curr = curr->next_child;
	curr->next_child = node;
	return node;
}

template<typename T, typename Match_Proc>
bool tree_node_has_cycle(Tree_Node<T>* node, T match_value, Match_Proc match)
{
	while (node->parent != nullptr)
	{
		node = node->parent;
	    if (match(node->value, match_value)) return true;
	}
	return false;
}

#endif
