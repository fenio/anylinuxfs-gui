#!/usr/bin/env node
// Cross-platform script to fix absolute paths in HTML files for Tauri
import { readFileSync, writeFileSync, readdirSync } from 'fs';
import { join } from 'path';

const buildDir = 'build';

// Find all HTML files in build directory
const htmlFiles = readdirSync(buildDir).filter(f => f.endsWith('.html'));

for (const file of htmlFiles) {
    const filePath = join(buildDir, file);
    let content = readFileSync(filePath, 'utf-8');

    // Replace absolute paths with relative paths
    content = content
        .replace(/href="\//g, 'href="./')
        .replace(/src="\//g, 'src="./')
        .replace(/import\("\//g, 'import("./');

    writeFileSync(filePath, content);
    console.log(`Fixed paths in ${file}`);
}
