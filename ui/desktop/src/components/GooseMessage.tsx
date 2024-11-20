import React from 'react'
import ToolInvocation from './ToolInvocation'
import ReactMarkdown from 'react-markdown'

// Schema-based content types
interface PlanStep {
  step: number;
  description: string;
}

interface Plan {
  name: string;
  steps: PlanStep[];
}

interface PlanApproval {
  type: 'PlanApproval';
  plan: Plan;
  confirmationRequired: boolean;
}

interface PlanSelection {
  type: 'PlanSelection';
  plans: Plan[];
}

interface ActionOption {
  name: string;
  description: string;
}

interface ActionOptions {
  type: 'ActionOptions';
  options: ActionOption[];
  selectionType: 'single' | 'multiple' | 'reject';
}

interface FormField {
  name: string;
  type: 'text' | 'textarea' | 'dropdown' | 'date' | 'number' | 'range';
  label: string;
  options?: string[];
  min?: number;
  max?: number;
  required?: boolean;
}

interface InputForm {
  type: 'InputForm';
  fields: FormField[];
}

interface ContentItem {
  type: 'text' | 'markdown' | 'link' | 'image';
  value: string;
  description?: string;
}

interface Presentation {
  type: 'Presentation';
  content: ContentItem[];
}

type SchemaContent = PlanApproval | PlanSelection | ActionOptions | InputForm | Presentation;

interface MessageMetadata {
  schemaContent: SchemaContent;
}

interface GooseMessageProps {
  message: any;
  metadata?: MessageMetadata;
}

const PlanStepDisplay: React.FC<{ step: PlanStep }> = ({ step }) => (
  <div className="flex items-start space-x-2 mb-2">
    <div className="font-bold min-w-[24px]">{step.step}.</div>
    <div>{step.description}</div>
  </div>
);

const PlanDisplay: React.FC<{ plan: Plan }> = ({ plan }) => (
  <div className="border rounded-lg p-4 mb-4">
    <h3 className="font-bold mb-2">{plan.name}</h3>
    <div className="space-y-2">
      {plan.steps.map((step) => (
        <PlanStepDisplay key={step.step} step={step} />
      ))}
    </div>
  </div>
);

const ActionOptionDisplay: React.FC<{ option: ActionOption }> = ({ option }) => (
  <div className="border rounded-lg p-3 mb-2 hover:bg-gray-50 cursor-pointer">
    <div className="font-semibold">{option.name}</div>
    <div className="text-sm text-gray-600">{option.description}</div>
  </div>
);

const FormFieldDisplay: React.FC<{ field: FormField }> = ({ field }) => (
  <div className="mb-4">
    <label className="block text-sm font-medium mb-1">
      {field.label}
      {field.required && <span className="text-red-500 ml-1">*</span>}
    </label>
    {field.type === 'dropdown' && field.options && (
      <select disabled className="w-full p-2 border rounded">
        <option value="">Select an option</option>
        {field.options.map((opt) => (
          <option key={opt} value={opt}>{opt}</option>
        ))}
      </select>
    )}
    {(field.type === 'text' || field.type === 'date') && (
      <input
        type={field.type}
        className="w-full p-2 border rounded"
        disabled
        placeholder={`Enter ${field.label.toLowerCase()}`}
      />
    )}
    {field.type === 'textarea' && (
      <textarea
        className="w-full p-2 border rounded"
        disabled
        placeholder={`Enter ${field.label.toLowerCase()}`}
        rows={3}
      />
    )}
  </div>
);

const ContentItemDisplay: React.FC<{ item: ContentItem }> = ({ item }) => {
  switch (item.type) {
    case 'markdown':
      return <ReactMarkdown>{item.value}</ReactMarkdown>;
    case 'image':
      return (
        <div className="mb-2">
          <img src={item.value} alt={item.description || ''} className="max-w-full rounded" />
          {item.description && <div className="text-sm text-gray-600 mt-1">{item.description}</div>}
        </div>
      );
    case 'link':
      return (
        <a href={item.value} target="_blank" rel="noopener noreferrer" className="text-blue-500 hover:underline">
          {item.description || item.value}
        </a>
      );
    default:
      return <div>{item.value}</div>;
  }
};

const SchemaContentDisplay: React.FC<{ content: SchemaContent }> = ({ content }) => {
  switch (content.type) {
    case 'PlanApproval':
      return (
        <div>
          <PlanDisplay plan={content.plan} />
          {content.confirmationRequired && (
            <div className="text-sm text-gray-600 mt-2">
              Please review and confirm this plan.
            </div>
          )}
        </div>
      );
    
    case 'PlanSelection':
      return (
        <div>
          <div className="mb-2 font-medium">Please select a plan:</div>
          {content.plans.map((plan, idx) => (
            <PlanDisplay key={idx} plan={plan} />
          ))}
        </div>
      );
    
    case 'ActionOptions':
      return (
        <div>
          <div className="mb-2 font-medium">
            {content.selectionType === 'single' ? 'Select one option:' :
             content.selectionType === 'multiple' ? 'Select one or more options:' :
             'Available options:'}
          </div>
          {content.options.map((option, idx) => (
            <ActionOptionDisplay key={idx} option={option} />
          ))}
        </div>
      );
    
    case 'InputForm':
      return (
        <div className="border rounded-lg p-4">
          {content.fields.map((field, idx) => (
            <FormFieldDisplay key={idx} field={field} />
          ))}
        </div>
      );
    
    case 'Presentation':
      return (
        <div className="space-y-4">
          {content.content.map((item, idx) => (
            <ContentItemDisplay key={idx} item={item} />
          ))}
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
              <div className="mt-4 border-t pt-4">
                <SchemaContentDisplay content={metadata.schemaContent} />
              </div>
            )}
          </div>
        )}
      </div>
    </div>
  )
}