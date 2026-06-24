import { useState } from "react"
import type { Invoice, InvoiceStatus } from "../types"
import { StatusBadge } from "./StatusBadge"
import { RefundConfirmationModal } from "./RefundConfirmationModal"

interface RefundRequestProps {
  invoice: Invoice
  walletAddress: string | null
  onRequestRefund: () => Promise<{
    success: boolean
    transaction_hash?: string
    error?: string
  }>
}

export function RefundRequest({
  invoice,
  walletAddress,
  onRequestRefund,
}: RefundRequestProps) {
  const [showConfirm, setShowConfirm] = useState(false)
  const [submitting, setSubmitting] = useState(false)
  const [result, setResult] = useState<{
    success: boolean
    hash?: string
    errorMsg?: string
  } | null>(null)

  const isPayer =
    walletAddress?.toLowerCase() === invoice.payer.toLowerCase()
  const canRequestRefund =
    isPayer && invoice.status === "Paid"

  const handleRefundClick = () => {
    setResult(null)
    setShowConfirm(true)
  }

  const handleConfirmRefund = async () => {
    setSubmitting(true)
    const res = await onRequestRefund()
    setSubmitting(false)
    setShowConfirm(false)
    setResult({
      success: res.success,
      hash: res.transaction_hash,
      errorMsg: res.error,
    })
  }

  return (
    <div className="refund-section">
      {result && (
        <div
          className={`message message--${result.success ? "success" : "error"}`}
        >
          {result.success ? (
            <>
              Refund requested successfully!
              <br />
              Transaction hash:{" "}
              <code className="tx-hash">{result.hash}</code>
            </>
          ) : (
            <>Refund request failed: {result.errorMsg}</>
          )}
        </div>
      )}

      {canRequestRefund && !result?.success && (
        <button className="btn btn--danger" onClick={handleRefundClick}>
          Request Refund
        </button>
      )}

      {invoice.status === "RefundRequested" && (
        <div className="status-info">
          <StatusBadge status={invoice.status as InvoiceStatus} />
          <p>Your refund request has been submitted and is being processed.</p>
        </div>
      )}

      {!canRequestRefund &&
        invoice.status !== "RefundRequested" &&
        isPayer && (
          <p className="status-text">
            Refund can only be requested on Paid invoices.
          </p>
        )}

      {showConfirm && (
        <RefundConfirmationModal
          invoice={invoice}
          onConfirm={handleConfirmRefund}
          onCancel={() => setShowConfirm(false)}
          submitting={submitting}
        />
      )}
    </div>
  )
}
