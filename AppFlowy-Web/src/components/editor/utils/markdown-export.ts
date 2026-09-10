import { Element, Node, Text } from 'slate';
import { BlockType } from '@/application/types';

function renderLeaf(leaf: Text): string {
  let text = leaf.text || '';
  if (!text) return '';

  if (leaf.code) {
    text = `\`${text}\``;
  }
  if (leaf.bold) {
    text = `**${text}**`;
  }
  if (leaf.italic) {
    text = `*${text}*`;
  }
  if (leaf.strikethrough) {
    text = `~~${text}~~`;
  }
  if (leaf.underline) {
    text = `<u>${text}</u>`;
  }
  if (leaf.href) {
    text = `[${text}](${leaf.href})`;
  }

  return text;
}

function renderInlineContent(node: Element): string {
  if (!node || !node.children) return '';

  return node.children
    .map((child) => {
      if (Text.isText(child)) {
        return renderLeaf(child);
      }
      if (Element.isElement(child)) {
        return renderInlineContent(child);
      }
      return '';
    })
    .join('');
}

function getPlainText(node: Element): string {
  return Node.string(node);
}

function renderBlock(node: Element, indent = 0): string {
  const indentStr = '  '.repeat(indent);
  const type = node.type as BlockType | string;
  const children = (node.children || []) as (Element | Text)[];

  // Find the inline text container (usually children[0] with type 'text')
  const textContainer = children.find((c) => Element.isElement(c) && c.type === 'text') as Element | undefined;
  const inline = textContainer ? renderInlineContent(textContainer) : '';
  const childBlocks = children.filter((c) => Element.isElement(c) && c.type !== 'text') as Element[];

  const renderNestedChildren = () => {
    return childBlocks.map((c) => renderBlock(c, indent + 1)).join('');
  };

  switch (type) {
    case BlockType.HeadingBlock: {
      const level = Math.min(Math.max((node.data as { level?: number })?.level || 1, 1), 6);
      const prefix = '#'.repeat(level);
      return `${indentStr}${prefix} ${inline}\n\n${renderNestedChildren()}`;
    }

    case BlockType.Paragraph: {
      const content = inline ? `${indentStr}${inline}\n\n` : '\n';
      return `${content}${renderNestedChildren()}`;
    }

    case BlockType.BulletedListBlock: {
      return `${indentStr}- ${inline}\n${renderNestedChildren()}`;
    }

    case BlockType.NumberedListBlock: {
      return `${indentStr}1. ${inline}\n${renderNestedChildren()}`;
    }

    case BlockType.TodoListBlock: {
      const checked = (node.data as { checked?: boolean })?.checked ? 'x' : ' ';
      return `${indentStr}- [${checked}] ${inline}\n${renderNestedChildren()}`;
    }

    case BlockType.ToggleListBlock: {
      const nested = renderNestedChildren();
      return `${indentStr}<details>\n${indentStr}<summary>${inline}</summary>\n${nested}${indentStr}</details>\n\n`;
    }

    case BlockType.QuoteBlock: {
      return `${indentStr}> ${inline}\n\n${renderNestedChildren()}`;
    }

    case BlockType.CodeBlock: {
      const lang = (node.data as { language?: string })?.language || '';
      const code = textContainer ? getPlainText(textContainer) : getPlainText(node);
      return `${indentStr}\`\`\`${lang}\n${code}\n${indentStr}\`\`\`\n\n${renderNestedChildren()}`;
    }

    case BlockType.DividerBlock: {
      return `${indentStr}---\n\n`;
    }

    case BlockType.ImageBlock: {
      const url = (node.data as { url?: string })?.url || '';
      return `${indentStr}![](${url})\n\n${renderNestedChildren()}`;
    }

    case BlockType.CalloutBlock: {
      return `${indentStr}> 💡 ${inline}\n\n${renderNestedChildren()}`;
    }

    case BlockType.Page: {
      return childBlocks.map((c) => renderBlock(c, indent)).join('');
    }

    default: {
      if (inline) {
        return `${indentStr}${inline}\n\n${renderNestedChildren()}`;
      }
      return renderNestedChildren();
    }
  }
}

/**
 * Serializes a Slate document root element to GitHub-flavored Markdown
 */
export function exportDocumentToMarkdown(slateRoot: Element, title?: string): string {
  let md = '';
  if (title) {
    md += `# ${title}\n\n`;
  }
  md += renderBlock(slateRoot, 0);
  return md.trim() + '\n';
}
