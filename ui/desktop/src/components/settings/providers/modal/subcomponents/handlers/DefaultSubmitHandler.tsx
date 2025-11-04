import { checkProvider } from '../../../../../../api';

/**
 * Standalone function to submit provider configuration
 * Useful for components that don't want to use the hook
 */
export const providerConfigSubmitHandler = async (
  upsertFn: (key: string, value: unknown, isSecret: boolean) => Promise<void>,
  provider: {
    name: string;
    metadata: {
      config_keys?: Array<{
        name: string;
        required?: boolean;
        default?: unknown;
        secret?: boolean;
      }>;
    };
  },
  configValues: Record<string, string>
) => {
  const parameters = provider.metadata.config_keys || [];

  const requiredParams = parameters.filter((param) => param.required);
  if (requiredParams.length === 0 && parameters.length > 0) {
    const allOptionalWithDefaults = parameters.every(
      (param) => !param.required && param.default !== undefined
    );
    if (allOptionalWithDefaults) {
      const promises: Promise<void>[] = [];

      for (const param of parameters) {
        if (param.default !== undefined) {
          const value =
            configValues[param.name] !== undefined ? configValues[param.name] : param.default;
          promises.push(upsertFn(param.name, value, param.secret === true));
        }
      }

      await Promise.all(promises);
      return;
    }
  }

  const upsertPromises = parameters.map(
    (parameter: { name: string; required?: boolean; default?: unknown; secret?: boolean }) => {
      // Skip parameters that don't have a value and aren't required
      if (!configValues[parameter.name] && !parameter.required) {
        return;
      }

      // For required parameters with no value, use the default if available
      const value =
        configValues[parameter.name] !== undefined
          ? configValues[parameter.name]
          : parameter.default;

      // Skip if there's still no value
      if (value === undefined || value === null) {
        return;
      }

      // Create the provider-specific config key
      const configKey = `${parameter.name}`;

      // Explicitly define is_secret as a boolean (true/false)
      const isSecret = parameter.secret === true;

      // Pass the is_secret flag from the parameter definition
      return upsertFn(configKey, value, isSecret);
    }
  );

  await Promise.all(upsertPromises);
  await checkProvider({
    body: { provider: provider.name },
    throwOnError: true,
  });
};
