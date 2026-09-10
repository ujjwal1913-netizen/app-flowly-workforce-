import { useCallback, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { toast } from 'sonner';
import * as Y from 'yjs';

import { BillingService } from '@/application/services/domains';
import { getCollab } from '@/application/services/js-services/http/collab-api';
import { getViewPdfBlob } from '@/application/services/js-services/http/export-api';
import { yDocToSlateContent } from '@/application/slate-yjs/utils/convert';
import { SubscriptionInterval, SubscriptionPlan, Types } from '@/application/types';
import { ReactComponent as DocIcon } from '@/assets/icons/doc.svg';
import { ReactComponent as FileIcon } from '@/assets/icons/file.svg';
import { ReactComponent as PDFIcon } from '@/assets/icons/pdf.svg';
import { useAppOverlayContext } from '@/components/app/app-overlay/AppOverlayContext';
import { useAppView, useCurrentWorkspaceId, useGetSubscriptions } from '@/components/app/app.hooks';
import { useSubscriptionPlan } from '@/components/app/hooks/useSubscriptionPlan';
import { exportDocumentToHtml } from '@/components/editor/utils/html-export';
import { exportDocumentToMarkdown } from '@/components/editor/utils/markdown-export';
import { Button } from '@/components/ui/button';
import { Switch } from '@/components/ui/switch';
import { downloadBlob } from '@/utils/download';

function ExportPanel({ viewId }: { viewId: string }) {
  const { t } = useTranslation();
  const view = useAppView(viewId);
  const viewIdResolved = view?.view_id;
  const workspaceId = useCurrentWorkspaceId();
  const getSubscriptions = useGetSubscriptions();
  const { isPro } = useSubscriptionPlan(getSubscriptions);
  const { showBlockingLoader, hideBlockingLoader } = useAppOverlayContext();
  const [linkedPagesOverride, setLinkedPagesOverride] = useState<boolean | null>(null);
  const includeLinkedPages = linkedPagesOverride ?? isPro;
  const [exporting, setExporting] = useState<boolean>(false);
  const exportingRef = useRef<boolean>(false);

  const handleExportPdf = useCallback(async () => {
    if (!workspaceId || !viewIdResolved || exportingRef.current) return;

    exportingRef.current = true;
    setExporting(true);
    showBlockingLoader(`${t('shareAction.exportPdfExporting')}...`);
    try {
      const { blob, filename } = await getViewPdfBlob(workspaceId, viewIdResolved, {
        includeNested: isPro ? includeLinkedPages : false,
        includeDatabase: isPro,
      });

      downloadBlob(blob, filename);
      toast.success(t('shareAction.exportPdfSuccess'));
    } catch (e) {
      const message = (e as { message?: string })?.message ?? t('shareAction.exportPdfError');

      toast.error(message);
    } finally {
      exportingRef.current = false;
      setExporting(false);
      hideBlockingLoader();
    }
  }, [workspaceId, viewIdResolved, isPro, includeLinkedPages, t, showBlockingLoader, hideBlockingLoader]);

  const handleExportMarkdown = useCallback(async () => {
    if (!workspaceId || !viewIdResolved || exportingRef.current) return;

    exportingRef.current = true;
    setExporting(true);
    showBlockingLoader('Exporting Markdown...');
    try {
      const collab = await getCollab(workspaceId, viewIdResolved, Types.Document);
      const doc = new Y.Doc();

      Y.applyUpdate(doc, collab.data);
      const slateRoot = yDocToSlateContent(doc);

      if (!slateRoot) {
        throw new Error('Could not parse document for export');
      }

      const title = view?.name || 'Untitled';
      const mdContent = exportDocumentToMarkdown(slateRoot, title);
      const blob = new Blob([mdContent], { type: 'text/markdown;charset=utf-8' });

      downloadBlob(blob, `${title}.md`);
      toast.success('Exported Markdown successfully');
    } catch (e) {
      const message = (e as { message?: string })?.message ?? 'Failed to export Markdown';

      toast.error(message);
    } finally {
      exportingRef.current = false;
      setExporting(false);
      hideBlockingLoader();
    }
  }, [workspaceId, viewIdResolved, view?.name, showBlockingLoader, hideBlockingLoader]);

  const handleExportHtml = useCallback(async () => {
    if (!workspaceId || !viewIdResolved || exportingRef.current) return;

    exportingRef.current = true;
    setExporting(true);
    showBlockingLoader('Exporting HTML...');
    try {
      const collab = await getCollab(workspaceId, viewIdResolved, Types.Document);
      const doc = new Y.Doc();

      Y.applyUpdate(doc, collab.data);
      const slateRoot = yDocToSlateContent(doc);

      if (!slateRoot) {
        throw new Error('Could not parse document for export');
      }

      const title = view?.name || 'Untitled';
      const htmlContent = exportDocumentToHtml(slateRoot, title);
      const blob = new Blob([htmlContent], { type: 'text/html;charset=utf-8' });

      downloadBlob(blob, `${title}.html`);
      toast.success('Exported HTML successfully');
    } catch (e) {
      const message = (e as { message?: string })?.message ?? 'Failed to export HTML';

      toast.error(message);
    } finally {
      exportingRef.current = false;
      setExporting(false);
      hideBlockingLoader();
    }
  }, [workspaceId, viewIdResolved, view?.name, showBlockingLoader, hideBlockingLoader]);

  const handleLinkedPagesChange = useCallback(
    async (checked: boolean) => {
      if (checked && !isPro) {
        if (!workspaceId) return;
        try {
          const link = await BillingService.getSubscriptionLink(
            workspaceId,
            SubscriptionPlan.Pro,
            SubscriptionInterval.Month,
          );

          window.open(link, '_blank');
          // eslint-disable-next-line
        } catch (e: any) {
          toast.error(e?.message ?? t('shareAction.exportPdfError'));
        }

        return;
      }

      setLinkedPagesOverride(checked);
    },
    [isPro, workspaceId, t],
  );

  return (
    <div className='flex flex-col items-stretch gap-4 px-4 py-4' data-testid='export-panel'>
      {/* Markdown Export */}
      <div className='flex items-center justify-between gap-4'>
        <div className='flex items-center gap-2'>
          <DocIcon className='h-5 w-5 text-text-primary' />
          <div className='flex flex-col'>
            <span className='text-sm text-text-primary'>Markdown (.md)</span>
            <span className='text-xs text-text-tertiary'>Export as GitHub-flavored Markdown</span>
          </div>
        </div>
        <Button
          size='sm'
          data-testid='export-markdown-button'
          onClick={handleExportMarkdown}
          disabled={exporting || !workspaceId || !view}
          loading={exporting}
        >
          Export
        </Button>
      </div>

      {/* HTML Export */}
      <div className='flex items-center justify-between gap-4'>
        <div className='flex items-center gap-2'>
          <FileIcon className='h-5 w-5 text-text-primary' />
          <div className='flex flex-col'>
            <span className='text-sm text-text-primary'>HTML (.html)</span>
            <span className='text-xs text-text-tertiary'>Export as styled standalone web page</span>
          </div>
        </div>
        <Button
          size='sm'
          data-testid='export-html-button'
          onClick={handleExportHtml}
          disabled={exporting || !workspaceId || !view}
          loading={exporting}
        >
          Export
        </Button>
      </div>

      <hr className='border-line-divider my-1' />

      {/* PDF Export */}
      <div className='flex items-center justify-between gap-4'>
        <div className='flex items-center gap-2'>
          <PDFIcon className='h-5 w-5' />
          <div className='flex flex-col'>
            <span className='text-sm text-text-primary'>{t('shareAction.exportPdf')}</span>
            <span className='text-xs text-text-tertiary'>{t('shareAction.exportPdfDescription')}</span>
          </div>
        </div>
        <Button
          size='sm'
          data-testid='export-pdf-button'
          onClick={handleExportPdf}
          disabled={exporting || !workspaceId || !view}
          loading={exporting}
        >
          {exporting ? t('shareAction.exportPdfExporting') : t('shareAction.exportPdf')}
        </Button>
      </div>

      <div className='flex items-center justify-between gap-4'>
        <div className='flex flex-col'>
          <span className='text-sm text-text-primary'>{t('shareAction.exportPdfIncludeLinkedPages')}</span>
          {!isPro && (
            <span className='text-xs text-text-tertiary'>{t('shareAction.exportPdfIncludeLinkedPagesPro')}</span>
          )}
        </div>
        <Switch
          data-testid='export-include-linked-pages-switch'
          checked={includeLinkedPages}
          onCheckedChange={handleLinkedPagesChange}
          disabled={exporting}
        />
      </div>
    </div>
  );
}

export default ExportPanel;
