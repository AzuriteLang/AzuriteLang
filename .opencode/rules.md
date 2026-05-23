# OpenCode Rules — AzuriteLang

## Conventions générales

- Langage : Rust, edition 2021
- Le projet est un workspace Cargo avec 4 crates :
  - `azurite_lexer` — analyse lexicale
  - `azurite_parser` — analyse syntaxique + AST
  - `azurite_codegen` — génération de code LLVM (inkwell)
  - `azurite_cli` — binaire CLI (clap)
- Toujours compiler avec `cargo check` avant de proposer du code.
- Les warnings Rust doivent être à zéro (sauf dead_code temporaire).
- Suivre le style Rust idiomatique (clippy, rustfmt).

## Règles de code

1. **Pas de `unwrap()`** — utiliser `?`, `.map_err()`, ou `expect()` avec un message explicite.
2. **Pas de panique en runtime** — les fonctions renvoient `Result<T, String>` ou `Result<T, Error>`.
3. **Documentation** — chaque module public a une docstring (`///`). Les types et fonctions publiques aussi.
4. **Tests** — chaque module a des tests unitaires (`#[cfg(test)]`). Toute nouvelle fonctionnalité doit avoir un test.
5. **Types forts** — utiliser des enums et structs, pas de strings pour représenter des concepts (tokens, noeuds AST, etc.).
6. **Nommage** :
   - Modules : `snake_case`
   - Types : `PascalCase`
   - Fonctions : `snake_case`
   - Variables : `snake_case`
   - Constantes : `SCREAMING_SNAKE_CASE`
7. **Séparation des responsabilités** — chaque crate a une responsabilité unique et claire. Pas de dépendances circulaires.
8. **Expressions vs Statements** — en AzuriteLang, tout est expression. Le parsing produit un AST où `Block`, `If`, `While` sont des expressions.
9. **Span** — chaque token et noeud AST important contient un `Span` (pos, line, column) pour des messages d'erreur précis.

## Workflow de développement

1. Implémenter dans le lexer d'abord (token → test)
2. Puis le parser (tokens → AST → test)
3. Puis le codegen (AST → LLVM IR → test)
4. Puis le CLI pour exposer la fonctionnalité
5. `cargo test` doit passer avant chaque commit
6. Ajouter un exemple `.az` dans `examples/` pour chaque nouveau feature

## Utilisation de l'IA

- L'IA doit toujours vérifier les fichiers existants avant d'en créer de nouveaux.
- L'IA doit lire les tests existants pour comprendre le style attendu.
- L'IA ne doit jamais supprimer de code existant sans explication.
- L'IA doit proposer des tests pour toute nouvelle fonctionnalité.
- L'IA doit suivre la structure workspace : ne pas ajouter de dépendances à un crate sans raison valable.
- L'IA doit mettre à jour le README du projet principal (`README.md`) et le README des libs concernées (`string`, `math`, etc.) à chaque ajout ou modification significative. Le README reflète l'état actuel des fonctionnalités, des builtins, et des libs standard.
