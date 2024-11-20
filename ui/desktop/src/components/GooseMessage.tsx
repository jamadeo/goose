import React from 'react'
import ToolInvocation from './ToolInvocation'
import ReactMarkdown from 'react-markdown'
import { Button } from './ui/button'

interface Plan {
  id: string;
  name: string;
  description: string;
}

interface SchemaContent {
  type: 'PlanConfirmation' | 'PlanChoice' | 'ComplexInput';
  plans?: Plan[];
  selectedPlan?: Plan;
  complexInputReason?: string;
}

interface MessageMetadata {
  schemaContent: SchemaContent;
}

interface GooseMessageProps {
  message: any;
  metadata?: MessageMetadata;
}

const PlanDisplay: React.FC<{ plan: Plan }> = ({ plan }) => (
  <div className="border rounded-lg p-4 mb-4">
    <h3 className="font-bold mb-2">{plan.name}</h3>
    <div className="text-sm">{plan.description}</div>
  </div>
);

const SchemaContentDisplay: React.FC<{ content: SchemaContent }> = ({ content }) => {
  const handleConfirm = () => {
    console.log('Plan confirmed:', content.selectedPlan || content.plans);
  };

  const handleCancel = () => {
    console.log('Action cancelled');
  };

  const handlePlanSelect = (plan: Plan) => {
    console.log('Plan selected:', plan);
  };

  switch (content.type) {
    case 'PlanConfirmation':
      return (
        <div>
          {content.selectedPlan && <PlanDisplay plan={content.selectedPlan} />}
          <div className="flex gap-2 mt-4">
            <Button onClick={handleConfirm}>Go</Button>
            <Button variant="outline" onClick={handleCancel}>Cancel</Button>
          </div>
        </div>
      );
    
    case 'PlanChoice':
      return (
        <div>
          <div className="mb-2 font-medium">Please select a plan:</div>
          {content.plans?.map((plan) => (
            <div 
              key={plan.id} 
              onClick={() => handlePlanSelect(plan)}
              className="cursor-pointer hover:bg-gray-50"
            >
              <PlanDisplay plan={plan} />
            </div>
          ))}
          <div className="flex gap-2 mt-4">
            <Button onClick={handleCancel}>Cancel</Button>
          </div>
        </div>
      );
    
    case 'ComplexInput':
      return (
        <div className="text-gray-600 italic">
          {content.complexInputReason}
        </div>
      );
    
    default:
      return null;
  }
};

export default function GooseMessage({ message, metadata }: GooseMessageProps) {
  return (
    <div className="flex mb-4 w-auto max-w-full">
      <div className="bg-goose-bubble text-black rounded-2xl p-4">
        {message.toolInvocations ? (
          <div className="space-y-4">
            {message.toolInvocations.map((toolInvocation: any) => (
              <ToolInvocation
                key={toolInvocation.toolCallId}
                toolInvocation={toolInvocation}
              />
            ))}
          </div>
        ) : (
          <div>
            <ReactMarkdown>{message.content}</ReactMarkdown>
            {metadata?.schemaContent && (
              <SchemaContentDisplay content={metadata.schemaContent} />
            )}
          </div>
        )}
      </div>
    </div>
  )
}