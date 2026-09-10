import { Element, Node, Text } from 'slate';
import { BlockType } from '@/application/types';

function escapeHtml(str: string): string {
  return str
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
    .replace(/"/g, '&quot;')
    .replace(/'/g, '&#039;');
}

function renderLeafHtml(leaf: Text): string {
  let text = escapeHtml(leaf.text || '');
  if (!text) return '';

  if (leaf.code) {
    text = `<code>${text}</code>`;
  }
  if (leaf.bold) {
    text = `<strong>${text}</strong>`;
  }
  if (leaf.italic) {
    text = `<em>${text}</em>`;
  }
  if (leaf.strikethrough) {
    text = `<del>${text}</del>`;
  }
  if (leaf.underline) {
    text = `<u>${text}</u>`;
  }
  if (leaf.href) {
    const href = escapeHtml(leaf.href);
    text = `<a href="${href}" target="_blank" rel="noopener noreferrer">${text}</a>`;
  }

  return text;
}

function renderInlineHtml(node: Element): string {
  if (!node || !node.children) return '';

  return node.children
    .map((child) => {
      if (Text.isText(child)) {
        return renderLeafHtml(child);
      }
      if (Element.isElement(child)) {
        return renderInlineHtml(child);
      }
      return '';
    })
    .join('');
}

function getPlainText(node: Element): string {
  return Node.string(node);
}

function renderBlockHtml(node: Element): string {
  const type = node.type as BlockType | string;
  const children = (node.children || []) as (Element | Text)[];

  const textContainer = children.find((c) => Element.isElement(c) && c.type === 'text') as Element | undefined;
  const inline = textContainer ? renderInlineHtml(textContainer) : '';
  const childBlocks = children.filter((c) => Element.isElement(c) && c.type !== 'text') as Element[];

  const renderNestedChildren = () => {
    return childBlocks.map((c) => renderBlockHtml(c)).join('\n');
  };

  switch (type) {
    case BlockType.HeadingBlock: {
      const level = Math.min(Math.max((node.data as { level?: number })?.level || 1, 1), 6);
      return `<h${level}>${inline}</h${level}>\n${renderNestedChildren()}`;
    }

    case BlockType.Paragraph: {
      const content = inline ? `<p>${inline}</p>` : '<p><br /></p>';
      return `${content}\n${renderNestedChildren()}`;
    }

    case BlockType.BulletedListBlock: {
      return `<ul><li>${inline}</li>\n${renderNestedChildren()}</ul>`;
    }

    case BlockType.NumberedListBlock: {
      return `<ol><li>${inline}</li>\n${renderNestedChildren()}</ol>`;
    }

    case BlockType.TodoListBlock: {
      const checked = (node.data as { checked?: boolean })?.checked ? 'checked' : '';
      return `<div class="todo-item"><input type="checkbox" ${checked} disabled /> <span>${inline}</span></div>\n${renderNestedChildren()}`;
    }

    case BlockType.ToggleListBlock: {
      const nested = renderNestedChildren();
      return `<details><summary>${inline}</summary><div class="toggle-content">${nested}</div></details>`;
    }

    case BlockType.QuoteBlock: {
      return `<blockquote><p>${inline}</p></blockquote>\n${renderNestedChildren()}`;
    }

    case BlockType.CodeBlock: {
      const lang = escapeHtml((node.data as { language?: string })?.language || '');
      const code = escapeHtml(textContainer ? getPlainText(textContainer) : getPlainText(node));
      return `<pre><code class="language-${lang}">${code}</code></pre>\n${renderNestedChildren()}`;
    }

    case BlockType.DividerBlock: {
      return `<hr />`;
    }

    case BlockType.ImageBlock: {
      const url = escapeHtml((node.data as { url?: string })?.url || '');
      return `<figure><img src="${url}" alt="" loading="lazy" /></figure>\n${renderNestedChildren()}`;
    }

    case BlockType.CalloutBlock: {
      return `<div class="callout"><span class="callout-icon">💡</span><div class="callout-body">${inline}</div></div>\n${renderNestedChildren()}`;
    }

    case BlockType.Page: {
      return childBlocks.map((c) => renderBlockHtml(c)).join('\n');
    }

    default: {
      if (inline) {
        return `<p>${inline}</p>\n${renderNestedChildren()}`;
      }
      return renderNestedChildren();
    }
  }
}

/**
 * Serializes a Slate document root element to a standalone, styled HTML document
 */
export function exportDocumentToHtml(slateRoot: Element, title = 'Document'): string {
  const bodyContent = renderBlockHtml(slateRoot);
  const safeTitle = escapeHtml(title);

  return `<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8">
  <meta name="viewport" content="width=device-width, initial-scale=1.0">
  <title>${safeTitle}</title>
  <style>
    :root {
      --bg: #ffffff;
      --text: #1f2328;
      --border: #d0d7de;
      --code-bg: #f6f8fa;
      --blockquote: #656d76;
    }
    @media (prefers-color-scheme: dark) {
      :root {
        --bg: #0d1117;
        --text: #e6edf3;
        --border: #30363d;
        --code-bg: #161b22;
        --blockquote: #8b949e;
      }
    }
    body {
      font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", "Noto Sans", Helvetica, Arial, sans-serif;
      line-height: 1.6;
      max-width: 820px;
      margin: 40px auto;
      padding: 0 24px;
      color: var(--text);
      background-color: var(--bg);
    }
    h1, h2, h3, h4, h5, h6 {
      margin-top: 24px;
      margin-bottom: 16px;
      font-weight: 600;
      line-height: 1.25;
    }
    h1 { font-size: 2em; border-bottom: 1px solid var(--border); padding-bottom: 0.3em; }
    h2 { font-size: 1.5em; border-bottom: 1px solid var(--border); padding-bottom: 0.3em; }
    p { margin-top: 0; margin-bottom: 16px; }
    code {
      font-family: ui-monospace, SFMono-Regular, "SF Mono", Menlo, Consolas, monospace;
      font-size: 85%;
      padding: 0.2em 0.4em;
      background-color: var(--code-bg);
      border-radius: 6px;
    }
    pre {
      padding: 16px;
      overflow: auto;
      font-size: 85%;
      line-height: 1.45;
      background-color: var(--code-bg);
      border-radius: 6px;
    }
    pre code { padding: 0; background: none; }
    blockquote {
      margin: 0 0 16px;
      padding: 0 1em;
      color: var(--blockquote);
      border-left: 0.25em solid var(--border);
    }
    hr {
      height: 0.25em;
      padding: 0;
      margin: 24px 0;
      background-color: var(--border);
      border: 0;
    }
    ul, ol { padding-left: 2em; margin-top: 0; margin-bottom: 16px; }
    li { margin-top: 0.25em; }
    .todo-item { display: flex; align-items: center; gap: 8px; margin: 4px 0; }
    .callout {
      display: flex;
      gap: 12px;
      padding: 12px 16px;
      margin: 16px 0;
      background-color: var(--code-bg);
      border-left: 4px solid #00BCF0;
      border-radius: 4px;
    }
    details { margin: 8px 0; }
    summary { cursor: pointer; font-weight: 500; }
    .toggle-content { padding-left: 16px; margin-top: 8px; }
    img { max-width: 100%; height: auto; border-radius: 6px; }
    a { color: #0969da; text-decoration: none; }
    a:hover { text-decoration: underline; }
  </style>
</head>
<body>
  <h1>${safeTitle}</h1>
  ${bodyContent}
</body>
</html>`;
}
