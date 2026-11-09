import js from '@eslint/js';
import globals from 'globals';

export default [
  {
    ignores: [
      'node_modules/**',
      'target/**',
      'coverage/**',
      'playwright-report/**',
      'test-results/**',
      'mutants.out*/**',
    ],
  },
  js.configs.recommended,
  {
    files: ['tests/**/*.js', 'tests/**/*.mjs', 'scripts/**/*.mjs', '*.config.js'],
    languageOptions: { globals: globals.node },
  },
  { files: ['tests/e2e/**/*.js'], languageOptions: { globals: globals.browser } },
  {
    files: ['web/**/*.js'],
    languageOptions: { sourceType: 'script', globals: globals.browser },
    // Legacy scripts share bindings across files. Other recommended correctness
    // rules remain active until these scripts have explicit module imports.
    rules: { 'no-undef': 'off', 'no-unused-vars': 'off' },
  },
  {
    rules: {
      'no-empty': ['error', { allowEmptyCatch: true }],
      'no-eval': 'error',
      'no-implied-eval': 'error',
    },
  },
];
