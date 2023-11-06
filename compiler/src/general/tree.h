#ifndef TREE_H
#define TREE_H

#include "arena.h"

template<typename T>
struct Tree_Node
{
	T value;
	Tree_Node* parent;
	Tree_Node* first_child;
	Tree_Node* next_sibling;
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
		root->first_child = nullptr;
		root->next_sibling = nullptr;
	}

	~Tree() { arena_deinit(&arena); }
};

template<typename T>
Tree_Node<T>* tree_node_add_child(Arena* arena, Tree_Node<T>* parent, T value)
{
	Tree_Node<T>* node = arena_alloc<Tree_Node<T>>(arena);
	node->value = value;
	node->parent = parent;
	node->first_child = nullptr;
	node->next_sibling = nullptr;

	if (parent->first_child == nullptr)
	{
		parent->first_child = node;
		return node;
	}
	
	Tree_Node<T>* sibling = parent->first_child;
	while (sibling->next_sibling != nullptr) sibling = sibling->next_sibling;
	sibling->next_sibling = node;
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

template<typename T, typename Apply_Proc, typename Context>
void tree_node_apply_proc_up_to_root(Tree_Node<T>* node, Context* context, Apply_Proc apply)
{
	std::vector<Tree_Node<T>*> cycle_nodes;

	while (node != nullptr)
	{
		cycle_nodes.emplace_back(node);
		node = node->parent;
	}

	for (auto it = cycle_nodes.rbegin(); it != cycle_nodes.rend(); it++) 
	{
		apply(context, *it);
	}
}

#endif
