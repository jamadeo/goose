import React from 'react'
import ToolInvocation from './ToolInvocation'
import ReactMarkdown from 'react-markdown'

interface MessageMetadata {
  randomText: string;
}

interface GooseMessageProps {
  message: any;
  metadata?: MessageMetadata;
}

export default function GooseMessage({ message, metadata }: GooseMessageProps) {
  return (
    <div className="flex mb-4 w-auto max-w-full">
      <div className="bg-goose-bubble text-black rounded-2xl p-4">
        {message.toolInvocations ? (
          <div className="space-y-4">
            {message.toolInvocations.map((toolInvocation) => (
              <ToolInvocation
                key={toolInvocation.toolCallId}
                toolInvocation={toolInvocation}
              />
            ))}
          </div>
        ) : (
          <div>
            <ReactMarkdown>{message.content}</ReactMarkdown>
            {metadata?.randomText && (
              <div className="mt-2 text-sm text-gray-500 italic">
                {metadata.randomText}
              </div>
            )}
          </div>
        )}
      </div>
    </div>
  )
}